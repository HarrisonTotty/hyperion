//! The `system_bodies` and `body_detail` requests and their answers (plan 14, P14.T36.b): the
//! checks of the requests' fields, and the wire form of a planetary system's zones and of its
//! bodies' records at a time.
//!
//! The records are the sim's (P14.T30.b's `snapshot_at` and `body_at`), degraded to the level the
//! request asked for (P14.T34), and every section keeps the state the sim tagged it with: the
//! server is the authority on what its generator version computes (ruling 34 of 2026-09-22), so
//! nothing is added or inferred here. Until the knowledge overlay exists, the level granted is the
//! level asked for.

use hyperion_protocol::{
    ArchitectureClassDto, BeltKindDto, BodyDetailDto, BodyDetailRequest, BodyHooksDto, BodyIdHex,
    BodyKindDto, BodyOrbitDto, BodyRecordDto, BodyStateDto, BodySummaryDto, BodySurfaceDto,
    BulkPropertiesDto, DestructionCauseDto, DetailLevelDto, ErrorCode, HabitableZoneDto,
    MassFractionsDto, MoonOriginDto, OrbitHostDto, PlanetClassDto, RequestError, SectionDto,
    SystemBodiesDto, SystemBodiesRequest, SystemPlaneDto, SystemSummaryDto, SystemSummaryRequest,
    ZoneDto,
};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::placement::Existence;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::architecture::{ArchitectureClass, HostMultiplicity};
use hyperion_sim::planetary::derive::{HabitableZone, PlanetClass};
use hyperion_sim::planetary::disc::snow_line;
use hyperion_sim::planetary::fate::{BodyState, DestructionCause};
use hyperion_sim::planetary::placement::classes::orbits::SystemPlane;
use hyperion_sim::planetary::placement::{OrbitHost, OrbitZone, ZoneDiscInputs};
use hyperion_sim::planetary::record::{
    BeltKind, BodyKind, BodyOrbit, BodyRecord, BulkProperties, DetailLevel, Hooks, MoonOrigin,
    Section, Surface,
};
use hyperion_sim::planetary::{BodyIndex, PlanetarySystem, ResolveBodyError, SystemContext};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{Kilograms, Metres};

use super::query_time;
use super::stellar::{orbit_dto, unknown_system, wire_time};

/// A `system_bodies` request, checked: the system it names, decoded, the time inside the clock
/// window, and the level asked for.
///
/// The checks are the `system_summary` request's, in its order (plan 06's P06.T34): the universe
/// before this, then `time` (`bad_request`), then `system`, whose bits must be a system ID
/// (`unknown_system`); the ID is resolved on the pool job that generates the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BodiesRequest {
    system: SystemId,
    time: UniverseTime,
    level: DetailLevel,
}

impl BodiesRequest {
    /// The system asked about, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(self) -> SystemId {
        self.system
    }

    /// The instant asked about, inside the clock window, which the planetary queries need.
    #[must_use]
    pub(crate) fn time(self) -> UniverseTime {
        self.time
    }

    /// The detail level granted: the one asked for.
    #[must_use]
    pub(crate) fn level(self) -> DetailLevel {
        self.level
    }
}

impl TryFrom<&SystemBodiesRequest> for BodiesRequest {
    type Error = RequestError;

    /// Checks `time`, then decodes `system`.
    fn try_from(request: &SystemBodiesRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let system = SystemId::from_raw(request.system.to_u64())
            .map_err(|error| unknown_system(&request.system, error))?;
        Ok(Self {
            system,
            time,
            level: detail_level(request.detail),
        })
    }
}

/// A `body_detail` request, checked: the body's system and index, decoded, the time inside the
/// clock window, and the level asked for.
///
/// The universe is looked up before this. Then `time` is checked (`bad_request` naming `time`),
/// then the body ID's two parts, both named `body`, the request's only field that holds them: bits
/// that are not a system ID name no system (`unknown_system`), and an index outside plan 14's
/// layout (design note 3, [`BodyIndex::decode`]) is malformed (`bad_request`). An ID whose text is
/// not a body ID's never reaches here: the request does not parse, and plan 04 answers
/// `bad_request`. The system is resolved, and the body looked up, on the pool job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DetailRequest {
    system: SystemId,
    index: BodyIndex,
    time: UniverseTime,
    level: DetailLevel,
}

impl DetailRequest {
    /// The body's system, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(self) -> SystemId {
        self.system
    }

    /// The body's index in its system, in plan 14's layout.
    #[must_use]
    pub(crate) fn index(self) -> BodyIndex {
        self.index
    }

    /// The instant asked about, inside the clock window.
    #[must_use]
    pub(crate) fn time(self) -> UniverseTime {
        self.time
    }

    /// The detail level granted: the one asked for.
    #[must_use]
    pub(crate) fn level(self) -> DetailLevel {
        self.level
    }
}

impl TryFrom<&BodyDetailRequest> for DetailRequest {
    type Error = RequestError;

    /// Checks `time`, then decodes the system and the index of `body`.
    fn try_from(request: &BodyDetailRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let (system, index) = request.body.to_parts();
        let system = SystemId::from_raw(system)
            .map_err(|error| unknown_body_system(&request.body, error))?;
        let index = BodyIndex::try_from(index).map_err(|error| {
            body_refusal(&request.body, ResolveBodyError::MalformedIndex(error))
        })?;
        Ok(Self {
            system,
            index,
            time,
            level: detail_level(request.detail),
        })
    }
}

/// Why the body a `body_detail` names cannot be answered, as the wire says it, on the field
/// `body`: `unknown_system` for a system that is none of this universe's, `bad_request` for an
/// index outside plan 14's layout, and `unknown_body` for an index the system holds no body at.
///
/// A star's index is `unknown_body` too: the stars are the hosts, whose records are plan 06's
/// `system_summary`, and no planetary record describes one (P14.T30.b).
#[must_use]
pub(crate) fn body_refusal(body: &BodyIdHex, error: ResolveBodyError) -> RequestError {
    let (code, message) = match error {
        ResolveBodyError::NoSuchSystem(error) => return unknown_body_system(body, error),
        ResolveBodyError::MalformedIndex(error) => {
            (ErrorCode::BadRequest, format!("invalid body: {error}"))
        }
        ResolveBodyError::NoSuchBody => (
            ErrorCode::UnknownBody,
            format!(
                "body {} names no planetary body of its system: its index is unused, or is a \
                 star's, whose record is the system summary's",
                body.as_str()
            ),
        ),
    };
    RequestError {
        code,
        message,
        field: Some("body".to_owned()),
    }
}

/// The refusal of a body ID whose system part names no system of the universe: `unknown_system`,
/// on the field `body`, which holds it.
#[must_use]
fn unknown_body_system(body: &BodyIdHex, reason: impl std::fmt::Display) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownSystem,
        message: format!(
            "body {} is of no system of this universe: {reason}",
            body.as_str()
        ),
        field: Some("body".to_owned()),
    }
}

/// The level a request asked for, as the sim names it.
#[must_use]
fn detail_level(level: DetailLevelDto) -> DetailLevel {
    match level {
        DetailLevelDto::Contact => DetailLevel::Contact,
        DetailLevelDto::MassAndOrbit => DetailLevel::MassAndOrbit,
        DetailLevelDto::Bulk => DetailLevel::Bulk,
        DetailLevelDto::Surface => DetailLevel::Surface,
        DetailLevelDto::Full => DetailLevel::Full,
    }
}

/// A detail level as the wire names it.
#[must_use]
fn detail_level_dto(level: DetailLevel) -> DetailLevelDto {
    match level {
        DetailLevel::Contact => DetailLevelDto::Contact,
        DetailLevel::MassAndOrbit => DetailLevelDto::MassAndOrbit,
        DetailLevel::Bulk => DetailLevelDto::Bulk,
        DetailLevel::Surface => DetailLevelDto::Surface,
        DetailLevel::Full => DetailLevelDto::Full,
    }
}

/// The request `system_summary` would have been for the same system and time, whose answer is
/// the hosts of a `system_bodies` answer.
#[must_use]
pub(crate) fn hosts_request(request: &SystemBodiesRequest) -> SystemSummaryRequest {
    SystemSummaryRequest {
        universe: request.universe.clone(),
        system: request.system.clone(),
        time: request.time,
    }
}

/// The answer to `system_bodies`: the hosts, the zones, the populations and every body's record at
/// the request's time, degraded to the level granted.
///
/// `hosts` is the `system_summary` answer for the same system and time. The zones and the system
/// plane are empty and `null` before the system is born, as `hosts` has no stars then; every body
/// is still listed, `not_yet_formed` (P14.T30.b).
///
/// # Panics
///
/// If `ctx` is not the context `planets` was generated from, or the time lies outside the clock
/// window, both of which the handler rules out (the pool job takes both from one cache entry, and
/// [`BodiesRequest`] checked the time).
#[must_use]
pub(crate) fn system_bodies(
    wanted: BodiesRequest,
    hosts: SystemSummaryDto,
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    seed: Seed,
) -> SystemBodiesDto {
    let t = wanted.time();
    let snapshot = planets.snapshot_at(ctx, t).degrade(wanted.level());
    let born = ctx.existence_at(t) == Existence::Exists;
    let zones: Vec<ZoneDto> = if born {
        zones(ctx, planets, seed, t)
    } else {
        Vec::new()
    };
    let system_plane = if born { system_plane(planets) } else { None };
    SystemBodiesDto {
        granted: detail_level_dto(snapshot.level()),
        hosts,
        zones,
        system_plane,
        belts: section(snapshot.belts(), |belts| {
            belts
                .iter()
                .map(|&index| body_id(planets.system(), index))
                .collect()
        }),
        halo: section(snapshot.halo(), |halo| {
            halo.map(|index| body_id(planets.system(), index))
        }),
        bodies: snapshot.bodies().iter().map(body_summary).collect(),
    }
}

/// The answer to `body_detail`: the record of one body at the request's time, degraded to the
/// level granted.
///
/// # Errors
///
/// [`body_refusal`] of [`ResolveBodyError::NoSuchBody`], `unknown_body`, if the system holds no
/// planetary body at the index.
///
/// # Panics
///
/// As [`system_bodies`].
pub(crate) fn body_detail(
    request: BodyDetailRequest,
    wanted: DetailRequest,
    ctx: &SystemContext,
    planets: &PlanetarySystem,
) -> Result<BodyDetailDto, RequestError> {
    let record = planets
        .body_at(ctx, wanted.index(), wanted.time())
        .map_err(|error| body_refusal(&request.body, error))?
        .degrade(wanted.level());
    Ok(BodyDetailDto {
        universe: request.universe,
        time: request.time,
        granted: detail_level_dto(record.level()),
        record: body_record(&record),
    })
}

/// Every zone of the system in the sim's order, hierarchy order and inside out, with what belongs
/// to its host at `t`.
#[must_use]
fn zones(
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    seed: Seed,
    t: UniverseTime,
) -> Vec<ZoneDto> {
    let stars = ctx.zone_stars();
    planets
        .zones()
        .iter()
        .zip(planets.hosts())
        .map(|(zone, host)| ZoneDto {
            host: orbit_host(planets.system(), zone.host()),
            inner_m: zone.inner().map(Metres::value),
            outer_m: zone.outer().map(Metres::value),
            snow_line_m: zone_snow_line(ctx, zone, &stars, seed).value(),
            plane: plane(host.plane()),
            architecture: architecture(host.class()),
            habitable_zone: planets
                .habitable_zone_at(ctx, zone.host(), t)
                .map(|zone| habitable_zone(&zone)),
        })
        .collect()
}

/// A zone's snow line: from its host's zero-age luminosity, summed over a pair's members (plan
/// 14, design note 6), by the expression its disc's is (`disc::derive`), so that a host whose disc
/// is cut to nothing still has one.
#[must_use]
fn zone_snow_line(
    ctx: &SystemContext,
    zone: &OrbitZone,
    stars: &[hyperion_sim::planetary::placement::ZoneStar],
    seed: Seed,
) -> Metres {
    let inputs = ZoneDiscInputs::for_zone(seed, ctx.id(), zone, stars, ctx.fe_h())
        .expect("a context gives every component a positive, finite zero-age state");
    snow_line(inputs.host().zams_luminosity())
}

/// The plane an orbit map of the whole system is drawn on (plan 14, design note 21): that of the
/// primary host's planets, the innermost zone about star 0, unless star 0 is a member of a close
/// binary, whose own plane its circumbinary zone shares; `None` for a system with no zone.
///
/// A primary that its companions leave no zone of its own takes the innermost zone that holds it,
/// and a system whose zones all leave it out, the first zone.
#[must_use]
fn system_plane(planets: &PlanetarySystem) -> Option<SystemPlaneDto> {
    let mut holding = planets
        .zones()
        .iter()
        .zip(planets.hosts())
        .filter(|(zone, _)| zone.members().any(|member| member == 0));
    let primary = match holding.next() {
        Some((zone, _)) if zone.host_multiplicity() == HostMultiplicity::CloseBinary => {
            holding.next()
        }
        innermost => innermost,
    };
    primary
        .map(|(_, host)| host)
        .or_else(|| planets.hosts().first())
        .map(|host| plane(host.plane()))
}

/// A body's ID on the wire.
#[must_use]
fn body_id(system: SystemId, index: BodyIndex) -> BodyIdHex {
    BodyIdHex::from_parts(system.raw(), index.get())
}

/// What a body or a zone orbits, as the wire says it.
#[must_use]
fn orbit_host(system: SystemId, host: OrbitHost) -> OrbitHostDto {
    match host {
        OrbitHost::Star(n) => OrbitHostDto::Star {
            body_index: u16::from(n),
        },
        OrbitHost::Pair(k) => OrbitHostDto::Pair {
            key_body_index: u16::from(k),
        },
        OrbitHost::Barycentre => OrbitHostDto::Barycentre,
        OrbitHost::Body(index) => OrbitHostDto::Body {
            id: body_id(system, index),
        },
    }
}

/// A section as the wire carries it, its value converted by `value`.
#[must_use]
fn section<T, U>(section: &Section<T>, value: impl FnOnce(&T) -> U) -> SectionDto<U> {
    match section {
        Section::Ok(inner) => SectionDto::Ok(value(inner)),
        Section::NotResolved => SectionDto::NotResolved,
        Section::NotModelled => SectionDto::NotModelled,
        Section::NotApplicable => SectionDto::NotApplicable,
    }
}

/// One body as a system's list carries it.
#[must_use]
fn body_summary(record: &BodyRecord) -> BodySummaryDto {
    let full = body_record(record);
    BodySummaryDto {
        id: full.id,
        kind: full.kind,
        label: full.label,
        parent: full.parent,
        state: full.state,
        position_m: full.position_m,
        mass_kg: full.mass_kg,
        orbit: full.orbit,
        moons: full.moons,
        rings: full.rings,
        bulk: full.bulk,
    }
}

/// One body's whole record as the wire carries it.
#[must_use]
fn body_record(record: &BodyRecord) -> BodyRecordDto {
    let identity = record.identity();
    let system = identity.system();
    let parent = identity.parent().map(|host| orbit_host(system, host));
    let ids = |indices: &Vec<BodyIndex>| {
        indices
            .iter()
            .map(|&index| body_id(system, index))
            .collect()
    };
    BodyRecordDto {
        id: body_id(system, identity.index()),
        kind: body_kind(identity.kind()),
        label: section(identity.label(), |label| label.as_str().to_owned()),
        parent: parent.clone(),
        state: body_state(identity.state()),
        position_m: record.position().map(|position| position.metres()),
        mass_kg: section(record.mass(), |&mass| Kilograms::from(mass).value()),
        orbit: section(record.orbit(), |orbit| body_orbit(parent.clone(), orbit)),
        moons: section(record.moons(), ids),
        rings: section(record.rings(), ids),
        bulk: section(record.bulk(), bulk),
        surface: section(record.surface(), |&value| surface(value)),
        hooks: section(record.hooks(), |&value| hooks(value)),
    }
}

/// A body's orbit, about `parent`, the host its identity names.
#[must_use]
fn body_orbit(parent: Option<OrbitHostDto>, orbit: &BodyOrbit) -> BodyOrbitDto {
    BodyOrbitDto {
        parent: parent.expect("a body with an orbit orbits a host"),
        orbit: orbit_dto(orbit.elements()),
        valid_until: orbit.valid_until().map(wire_time),
    }
}

/// A body's bulk section as the wire carries it, in SI units.
#[must_use]
fn bulk(bulk: &BulkProperties) -> BulkPropertiesDto {
    let fractions = bulk.fractions();
    BulkPropertiesDto {
        radius_m: Metres::from(bulk.radius()).value(),
        density_kg_m3: bulk.density().value(),
        surface_gravity_m_s2: bulk.surface_gravity().value(),
        class: planet_class(bulk.class()),
        mass_fractions: MassFractionsDto {
            iron: fractions.iron(),
            rock: fractions.rock(),
            water: fractions.water(),
            envelope: fractions.envelope(),
        },
        equilibrium_temperature_k: bulk.equilibrium_temperature().value(),
    }
}

/// A surface section's value, which no record of this generator version holds.
#[must_use]
fn surface(surface: Surface) -> BodySurfaceDto {
    match surface {}
}

/// A hooks section's value, which no record of this generator version holds.
#[must_use]
fn hooks(hooks: Hooks) -> BodyHooksDto {
    match hooks {}
}

/// What a body is, as the wire says it.
#[must_use]
fn body_kind(kind: BodyKind) -> BodyKindDto {
    match kind {
        BodyKind::Planet => BodyKindDto::Planet,
        BodyKind::DwarfPlanet => BodyKindDto::DwarfPlanet,
        BodyKind::Moon(origin) => BodyKindDto::Moon {
            origin: match origin {
                MoonOrigin::Regular => MoonOriginDto::Regular,
                MoonOrigin::GiantImpact => MoonOriginDto::GiantImpact,
                MoonOrigin::Captured => MoonOriginDto::Captured,
            },
        },
        BodyKind::Ring => BodyKindDto::Ring,
        BodyKind::Belt(belt) => BodyKindDto::Belt {
            belt_kind: match belt {
                BeltKind::Asteroid => BeltKindDto::Asteroid,
                BeltKind::Kuiper => BeltKindDto::Kuiper,
            },
        },
        BodyKind::CometaryHalo => BodyKindDto::CometaryHalo,
        BodyKind::ProtoplanetaryDisc => BodyKindDto::ProtoplanetaryDisc,
        BodyKind::DebrisDisc => BodyKindDto::DebrisDisc,
        BodyKind::Unresolved => BodyKindDto::Unresolved,
    }
}

/// What has become of a body, as the wire says it.
#[must_use]
fn body_state(state: BodyState) -> BodyStateDto {
    match state {
        BodyState::NotYetFormed => BodyStateDto::NotYetFormed,
        BodyState::Present => BodyStateDto::Present,
        BodyState::Destroyed { cause, at } => BodyStateDto::Destroyed {
            cause: match cause {
                DestructionCause::Dispersed => DestructionCauseDto::Dispersed,
                DestructionCause::Engulfed => DestructionCauseDto::Engulfed,
                DestructionCause::TidallyDisrupted => DestructionCauseDto::TidallyDisrupted,
                DestructionCause::Collided => DestructionCauseDto::Collided,
            },
            at: wire_time(at),
        },
        BodyState::Unbound { at } => BodyStateDto::Unbound { at: wire_time(at) },
    }
}

/// A planet's class, as the wire says it.
#[must_use]
fn planet_class(class: PlanetClass) -> PlanetClassDto {
    match class {
        PlanetClass::Rocky => PlanetClassDto::Rocky,
        PlanetClass::Icy => PlanetClassDto::Icy,
        PlanetClass::SubNeptune => PlanetClassDto::SubNeptune,
        PlanetClass::IceGiant => PlanetClassDto::IceGiant,
        PlanetClass::GasGiant => PlanetClassDto::GasGiant,
    }
}

/// An architecture class, as the wire says it.
#[must_use]
fn architecture(class: ArchitectureClass) -> ArchitectureClassDto {
    match class {
        ArchitectureClass::Barren => ArchitectureClassDto::Barren,
        ArchitectureClass::TerrestrialOnly => ArchitectureClassDto::TerrestrialOnly,
        ArchitectureClass::CompactMulti => ArchitectureClassDto::CompactMulti,
        ArchitectureClass::CompactWithColdGiant => ArchitectureClassDto::CompactWithColdGiant,
        ArchitectureClass::SolarLike => ArchitectureClassDto::SolarLike,
        ArchitectureClass::EccentricGiant => ArchitectureClassDto::EccentricGiant,
        ArchitectureClass::WarmGiant => ArchitectureClassDto::WarmGiant,
        ArchitectureClass::HotJupiter => ArchitectureClassDto::HotJupiter,
        ArchitectureClass::SubstellarCompact => ArchitectureClassDto::SubstellarCompact,
    }
}

/// A host's planets' plane, in the system frame.
#[must_use]
fn plane(plane: SystemPlane) -> SystemPlaneDto {
    SystemPlaneDto {
        inclination_rad: plane.inclination().value(),
        ascending_node_rad: plane.node().value(),
    }
}

/// A habitable zone's limits in metres, each `null` where the sim puts it at infinity, since JSON
/// has none.
#[must_use]
fn habitable_zone(zone: &HabitableZone) -> HabitableZoneDto {
    let finite = |limit: Metres| Some(limit.value()).filter(|m| m.is_finite());
    HabitableZoneDto {
        recent_venus_m: finite(zone.recent_venus()),
        runaway_greenhouse_m: finite(zone.runaway_greenhouse()),
        moist_greenhouse_m: finite(zone.moist_greenhouse()),
        maximum_greenhouse_m: finite(zone.maximum_greenhouse()),
        early_mars_m: finite(zone.early_mars()),
        extrapolated: zone.extrapolated(),
    }
}
