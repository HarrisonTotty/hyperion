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
    AbsorberDto, AerosolInventoryDto, AerosolModeDto, AerosolNotModelledDto, AerosolShapeDto,
    AerosolTauBasisDto, ArchitectureClassDto, BandsDto, BeltComponentDto, BeltCompositionDto,
    BeltDto, BeltGapDto, BeltKindDto, BeltSiteDto, BodyDetailDto, BodyDetailRequest,
    BodyEnvelopeDto, BodyFigureDto, BodyGrantDto, BodyHooksDto, BodyIdHex, BodyKindDto,
    BodyOrbitDto, BodyPhotometryDto, BodyRecordDto, BodyRotationDto, BodyStateDto, BodySummaryDto,
    BodySurfaceDto, BulkPropertiesDto, ClimateForcingDto, ClimateRegimeDto, CometaryHaloDto,
    CondensateDeckDto, CraterContractDto, CraterScreeningDto, CraterTargetClassDto,
    CrustCompositionDto, CrustLidDto, DestructionCauseDto, DetailLevelDto, DetailSeedHex,
    ErrorCode, FigureDatumDto, FigureLawDto, GasShareDto, HabitableZoneDto, MagneticFieldClassDto,
    MassFractionsDto, MixingRatioNodeDto, Modelled, MoonOriginDto, NotFormedCondensateDto,
    NotFormedReasonDto, OceanFloorDto, OrbitDriftDto, OrbitHostDto, PhaseTemplateDto,
    PlanetClassDto, PopulationDto, ProfileNodeDto, RadiationClassDto, RedoxClassDto, RequestError,
    RingDto, RingGapDto, RingKindDto, RingMaterialDto, SceneBodyDto, SceneSystemDto, SectionDto,
    SeenPositionDto, SizeDistributionDto, SpinResonanceDto, SubstanceAreaDto, SubstancePhaseDto,
    SubsurfaceOceanDto, SurfaceCondensateDto, SurfaceMaterialDto, SurfaceModifiersDto,
    SurfaceStateDto, SystemBodiesDto, SystemBodiesRequest, SystemPlaneDto, SystemSummaryDto,
    SystemSummaryRequest, TectonicRegimeDto, ThermalRegimeDto, VerticalStructureDto, WetEpochDto,
    ZoneDto,
};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::placement::Existence;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::architecture::{ArchitectureClass, HostMultiplicity};
use hyperion_sim::planetary::belts::{BeltComposition, BeltSite};
use hyperion_sim::planetary::chem::condense::{NotFormed, ResolveOpticalDepthError, SizeModeError};
use hyperion_sim::planetary::chem::partition::{CondensatePhase, Condensates};
use hyperion_sim::planetary::chem::redox::RedoxClass;
use hyperion_sim::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use hyperion_sim::planetary::derive::figure::{BodyFigure, Datum, FigureLaw};
use hyperion_sim::planetary::derive::photometry::{Bands, BodyPhotometry, PhaseTemplate};
use hyperion_sim::planetary::derive::rotation::SpinOrbitResonance;
use hyperion_sim::planetary::derive::{HabitableZone, PlanetClass};
use hyperion_sim::planetary::disc::snow_line;
use hyperion_sim::planetary::fate::{BodyState, DestructionCause};
use hyperion_sim::planetary::frames::BodyFixedFrame;
use hyperion_sim::planetary::hooks::aerosols::{
    Absorber, AbsorberLayer, AbsorberNode, AerosolDeck, AerosolMode, DepthBasis, ModeDepth,
    ParticleSize, ShapeClass,
};
use hyperion_sim::planetary::hooks::crust::{CrustComposition, LidCrust};
use hyperion_sim::planetary::hooks::figures::WetEpoch;
use hyperion_sim::planetary::hooks::surface::regime::{Forcing, ThermalRegime};
use hyperion_sim::planetary::hooks::{
    AerosolInventory, ClimateRegime, MagneticField, OceanFloor, RadiationClass, Subsurface,
    SubsurfaceOcean, SurfaceModifiers, TectonicRegime, VerticalStructure,
};
use hyperion_sim::planetary::placement::classes::orbits::SystemPlane;
use hyperion_sim::planetary::placement::{OrbitHost, OrbitZone, ZoneDiscInputs};
use hyperion_sim::planetary::record::{
    BeltKind, BodyKind, BodyOrbit, BodyRecord, BulkProperties, DetailLevel, Envelope, Hooks,
    Modelled as RecordModelled, MoonOrigin, Population, Section, Surface, SystemSnapshot,
};
use hyperion_sim::planetary::rings::{Ring, RingKind, RingMaterial};
use hyperion_sim::planetary::{
    BodyIndex, BodySub, PlanetarySystem, ResolveBodyError, SystemContext,
};
use hyperion_sim::substance::{
    CondensedPhase, ExtinctionBasis, ResolveAerosolError, SizeDistributionKind, SubstanceId,
};
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, Metres, MetresPerSecond, Seconds,
};
use hyperion_surface::craters::{CraterParams, Screening, TargetClass};

use super::query_time;
use super::stellar::{orbit_dto, unknown_system, wire_time};
use super::surface::climate_model;

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

impl BodiesRequest {
    /// The request for `system` at `time` at `level`: the scene's own, which reaches no wire
    /// request.
    ///
    /// # Panics
    ///
    /// If `time` lies outside the clock window, which the scene clock never leaves.
    #[must_use]
    pub(crate) fn new(system: SystemId, time: UniverseTime, level: DetailLevel) -> Self {
        assert!(
            ClockWindow::contains(time),
            "the scene's time {time} lies outside the clock window"
        );
        Self {
            system,
            time,
            level,
        }
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
pub(super) fn unknown_body_system(
    body: &BodyIdHex,
    reason: impl std::fmt::Display,
) -> RequestError {
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
pub(crate) fn detail_level(level: DetailLevelDto) -> DetailLevel {
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
pub(crate) fn detail_level_dto(level: DetailLevel) -> DetailLevelDto {
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
        mode: hyperion_protocol::QueryModeDto::Now,
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
    let snapshot = planets
        .snapshot_at(ctx, wanted.time())
        .degrade(wanted.level());
    let bodies = snapshot.bodies().iter().map(body_summary).collect();
    assemble(wanted, hosts, ctx, planets, seed, &snapshot, bodies)
}

/// One body of a scene's system as [`scene_system`] lists it: its index, the level it was
/// granted, and when its elements stop holding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ListedBody {
    /// The body's index.
    pub(crate) index: BodyIndex,
    /// The level granted for it.
    pub(crate) level: DetailLevel,
    /// When its elements stop holding, from its whole record whatever the level shows: the scene
    /// time at which the scene re-sends it.
    pub(crate) valid_until: Option<UniverseTime>,
}

/// The scene's system (rendering plan R03, Design note 13): plan 14's [`system_bodies`] answer at
/// the level asked, with each body's record degraded to its own grant, and the grants beside it.
///
/// `grant` gives each body's level, held to the level asked, which [`BodyRecord::degrade`] never
/// exceeds; a body its level does not resolve (a belt's member below `bulk`) is left out, as
/// [`SystemSnapshot::degrade`](hyperion_sim::planetary::record::SystemSnapshot::degrade) leaves it
/// out. A body granted `contact` carries `seen_from_ship`'s position. The system's own `granted`
/// states the level asked, the most any record holds. Returns the system and its bodies as listed,
/// in index order.
///
/// The system's `place` is left `None`: the caller, `SceneCore::arrive`, sets it from the galaxy
/// through [`system_place`](super::system_place), so that this converter takes no galaxy (R03.T16).
/// It is the one caller, and its tests hold that the state and every arrival carry the place.
/// # Panics
///
/// As [`system_bodies`].
#[must_use]
pub(crate) fn scene_system(
    wanted: BodiesRequest,
    hosts: SystemSummaryDto,
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    seed: Seed,
    mut grant: impl FnMut(BodyIndex) -> DetailLevel,
    mut seen_from_ship: impl FnMut(BodyIndex) -> Option<SeenPositionDto>,
) -> (SceneSystemDto, Vec<ListedBody>) {
    let full = planets.snapshot_at(ctx, wanted.time());
    let snapshot = full.degrade(wanted.level());
    let system = planets.system();
    let mut bodies = Vec::with_capacity(full.bodies().len());
    let mut grants = Vec::with_capacity(full.bodies().len());
    let mut listed = Vec::with_capacity(full.bodies().len());
    for record in full.bodies() {
        let index = record.index();
        let level = grant(index).min(wanted.level());
        if !level.resolves(index) {
            continue;
        }
        let sighting = if level == DetailLevel::Contact {
            seen_from_ship(index)
        } else {
            None
        };
        bodies.push(body_summary(&record.degrade(level)));
        grants.push(BodyGrantDto {
            body: body_id(system, index),
            level: detail_level_dto(level),
            seen: sighting,
        });
        listed.push(ListedBody {
            index,
            level,
            valid_until: valid_until(record),
        });
    }
    let system = assemble(wanted, hosts, ctx, planets, seed, &snapshot, bodies);
    (
        SceneSystemDto {
            system,
            grants,
            place: None,
        },
        listed,
    )
}

/// One body of a scene re-sent at `t` at `level`, with `seen` for a contact, and when its elements
/// stop holding; `None` if `level` does not resolve the body, or the system holds none at `index`.
///
/// # Panics
///
/// As [`system_bodies`].
#[must_use]
pub(crate) fn scene_body(
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    index: BodyIndex,
    t: UniverseTime,
    level: DetailLevel,
    seen: Option<SeenPositionDto>,
) -> Option<(SceneBodyDto, ListedBody)> {
    if !level.resolves(index) {
        return None;
    }
    let record = planets.body_at(ctx, index, t).ok()?;
    let listed = ListedBody {
        index,
        level,
        valid_until: valid_until(&record),
    };
    let body = SceneBodyDto {
        level: detail_level_dto(level),
        record: body_summary(&record.degrade(level)),
        seen: if level == DetailLevel::Contact {
            seen
        } else {
            None
        },
    };
    Some((body, listed))
}

/// When a whole record's elements stop holding, if they do inside the clock window.
#[must_use]
fn valid_until(record: &BodyRecord) -> Option<UniverseTime> {
    record.orbit().ok().and_then(BodyOrbit::valid_until)
}

/// A `system_bodies` answer of `snapshot`, degraded to the level asked, with `bodies` as its list.
#[must_use]
fn assemble(
    wanted: BodiesRequest,
    hosts: SystemSummaryDto,
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    seed: Seed,
    snapshot: &SystemSnapshot,
    bodies: Vec<BodySummaryDto>,
) -> SystemBodiesDto {
    let t = wanted.time();
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
        bodies,
    }
}

/// The answer to `body_detail`: the record of one body at the request's time, degraded to the
/// level granted.
///
/// # Errors
///
/// [`body_refusal`] of [`ResolveBodyError::NoSuchBody`], `unknown_body`, if the system holds no
/// planetary body at the index, or the level asked for does not resolve the body there
/// ([`DetailLevel::resolves`]: a belt's member, or a member's moon, below `bulk`). The two
/// refusals are word for word the same, so that a refusal does not confirm that a withheld member
/// exists, and `body_detail` resolves only what `system_bodies` lists (ruling 113.4).
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
    if !wanted.level().resolves(wanted.index()) {
        return Err(body_refusal(&request.body, ResolveBodyError::NoSuchBody));
    }
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

/// One body's whole record as the wire carries it: its list entry ([`body_summary`]), with its
/// surface, envelope and hooks.
#[must_use]
fn body_record(record: &BodyRecord) -> BodyRecordDto {
    let summary = body_summary(record);
    BodyRecordDto {
        id: summary.id,
        kind: summary.kind,
        label: summary.label,
        parent: summary.parent,
        state: summary.state,
        position_m: summary.position_m,
        mass_kg: summary.mass_kg,
        orbit: summary.orbit,
        moons: summary.moons,
        rings: summary.rings,
        population: summary.population,
        bulk: summary.bulk,
        rotation: summary.rotation,
        figure: summary.figure,
        photometry: summary.photometry,
        surface: section(record.surface(), surface),
        envelope: Some(section(record.envelope(), |inner| envelope(*inner))),
        hooks: section(record.hooks(), hooks),
    }
}

/// One body as a system's list carries it: every section of its record but the surface, the
/// envelope and the hooks, which the list does not convert.
#[must_use]
fn body_summary(record: &BodyRecord) -> BodySummaryDto {
    let identity = record.identity();
    let system = identity.system();
    let parent = identity.parent().map(|host| orbit_host(system, host));
    let ids = |indices: &Vec<BodyIndex>| {
        indices
            .iter()
            .map(|&index| body_id(system, index))
            .collect()
    };
    BodySummaryDto {
        id: body_id(system, identity.index()),
        kind: body_kind(identity.kind()),
        label: section(identity.label(), |label| label.as_str().to_owned()),
        parent: parent.clone(),
        state: body_state(identity.state()),
        position_m: record.position().map(|position| position.metres()),
        mass_kg: section(record.mass(), |&mass| Kilograms::from(mass).value()),
        orbit: section(record.orbit(), |orbit| {
            body_orbit(system, parent.clone(), orbit)
        }),
        moons: section(record.moons(), ids),
        rings: section(record.rings(), ids),
        population: section(record.population(), |population| {
            population_dto(system, population)
        }),
        bulk: section(record.bulk(), bulk),
        rotation: Some(section(record.rotation(), rotation)),
        figure: Some(section(record.figure(), figure)),
        photometry: Some(section(record.photometry(), photometry)),
    }
}

/// A body's orbit, about `parent`, the host its identity names, or the host the orbit says it
/// is about where that differs (a belt's member, listed under its belt, orbits the belt's host).
#[must_use]
fn body_orbit(system: SystemId, parent: Option<OrbitHostDto>, orbit: &BodyOrbit) -> BodyOrbitDto {
    BodyOrbitDto {
        parent: orbit
            .host()
            .map(|host| orbit_host(system, host))
            .or(parent)
            .expect("a body with an orbit orbits a host"),
        orbit: orbit_dto(orbit.elements()),
        valid_until: orbit.valid_until().map(wire_time),
        drift: orbit.drift().map(|drift| OrbitDriftDto {
            reference: wire_time(drift.reference()),
            semi_major_axis_rate_m_per_s: drift.semi_major_axis_rate_m_per_s(),
            eccentricity_rate_per_s: drift.eccentricity_rate_per_s(),
            mean_motion_rate_rad_per_s2: drift.mean_motion_rate_rad_per_s2(),
        }),
    }
}

/// A population's extent as the wire carries it (P14.T35.b's `BeltDto` and the halo section).
#[must_use]
fn population_dto(system: SystemId, population: &Population) -> PopulationDto {
    let ids = |indices: &Vec<BodyIndex>| {
        indices
            .iter()
            .map(|&index| body_id(system, index))
            .collect::<Vec<_>>()
    };
    match population {
        Population::Ring(ring) => PopulationDto::Ring(RingDto {
            ring_kind: match ring.kind() {
                RingKind::Dusty => RingKindDto::Dusty,
                RingKind::Massive => RingKindDto::Massive,
            },
            material: match ring.material() {
                RingMaterial::PorousIce => RingMaterialDto::PorousIce,
                RingMaterial::Rock => RingMaterialDto::Rock,
            },
            inner_edge_m: ring.inner_edge().value(),
            outer_edge_m: ring.outer_edge().value(),
            optical_depth: ring.optical_depth(),
            gaps: ring
                .gaps()
                .iter()
                .map(|gap| {
                    let (outer, inner) = gap.resonance();
                    RingGapDto {
                        moon: ring_gap_moon(system, ring, gap.moon()),
                        resonance: [outer, inner],
                        radius_m: gap.radius().value(),
                    }
                })
                .collect(),
            // P14.T56.a: the plane of a ring on its planet's Laplace surface, sent for every ring;
            // the field stays optional on the wire for older servers (decision-p14-t56a §1).
            normal: Some(ring.normal()),
        }),
        Population::Belt(belt) => {
            let component = |(inner, outer): (Metres, Metres)| BeltComponentDto {
                inner_edge_m: inner.value(),
                outer_edge_m: outer.value(),
            };
            PopulationDto::Belt(BeltDto {
                host: orbit_host(system, belt.host()),
                site: match belt.site() {
                    BeltSite::InsideGiant => BeltSiteDto::InsideGiant,
                    BeltSite::Gap => BeltSiteDto::Gap,
                    BeltSite::BeyondPlanets => BeltSiteDto::BeyondPlanets,
                    BeltSite::OuterDisc => BeltSiteDto::OuterDisc,
                },
                inner_edge_m: belt.inner_edge().value(),
                outer_edge_m: belt.outer_edge().value(),
                main: component(belt.main()),
                scattered: belt.scattered().map(component),
                gaps: belt
                    .gaps()
                    .iter()
                    .map(|gap| {
                        let (outer, inner) = gap.resonance();
                        BeltGapDto {
                            resonance: [outer, inner],
                            radius_m: gap.radius().value(),
                        }
                    })
                    .collect(),
                size_slope: belt.size_slope(),
                largest_diameter_m: belt.largest_diameter().value(),
                composition: match belt.composition() {
                    BeltComposition::Rocky => BeltCompositionDto::Rocky,
                    BeltComposition::Icy => BeltCompositionDto::Icy,
                },
                mean_eccentricity: belt.mean_eccentricity(),
                mean_inclination_rad: belt.mean_inclination().value(),
                fractional_luminosity: belt.fractional_luminosity(),
                members: section(belt.members(), ids),
            })
        }
        Population::CometaryHalo(halo) => PopulationDto::CometaryHalo(CometaryHaloDto {
            host: orbit_host(system, halo.host()),
            inner_edge_m: halo.inner_edge().value(),
            outer_edge_m: halo.outer_edge().value(),
            comets: halo.comets(),
            comet_rate_per_s: halo.comet_rate().value(),
        }),
    }
}

/// The moon whose resonance clears a ring's gap: the `n`-th regular moon of the ring's planet,
/// counted from 0 inside out, which is sub-index n + 1 (P14.T22.a numbers the regular moons
/// first).
#[must_use]
fn ring_gap_moon(system: SystemId, ring: &Ring, n: usize) -> BodyIdHex {
    let planet = ring.index().parent().expect("a ring belongs to its planet");
    let sub = u8::try_from(n + 1).expect("a planet has at most six regular moons");
    let moon = BodyIndex::new(planet.slot(), BodySub::Moon(sub))
        .expect("a planet's regular moons are in its moon block");
    body_id(system, moon)
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
        effective_temperature_k: bulk.effective_temperature().map(Kelvin::value),
    }
}

/// A body's frame and rotation law as the wire carries them, every parameter of `W(t)`
/// ([`RotationLawParts`](hyperion_sim::planetary::derive::rotation::RotationLawParts)).
#[must_use]
fn rotation(frame: &BodyFixedFrame) -> BodyRotationDto {
    let law = frame.rate().parts();
    BodyRotationDto {
        pole: frame.pole(),
        equator_node: frame.equator_node(),
        equator_quarter: frame.equator_quarter(),
        obliquity_rad: frame.obliquity().value(),
        initial_rate_rad_s: law.initial_rate,
        locked_rate_rad_s: law.locked_rate,
        age_at_epoch_s: law.age_at_epoch.value(),
        locking_age_s: law.locking_age.map(Seconds::value),
        locks_at: law.locks_at.map(wire_time),
        resonance: match law.resonance {
            SpinOrbitResonance::Synchronous => SpinResonanceDto::Synchronous,
            SpinOrbitResonance::ThreeToTwo => SpinResonanceDto::ThreeToTwo,
        },
        clock_period_s: law.clock_period.value(),
        clock_mean_anomaly_at_epoch_rad: law.clock_mean_anomaly_at_epoch.value(),
        sub_primary_angle_rad: law.sub_primary_angle.value(),
        phase_at_epoch_rad: law.phase_at_epoch.value(),
        capture_phase_rad: law.capture_phase.value(),
    }
}

/// A body's figure as the wire carries it, in metres.
#[must_use]
fn figure(figure: &BodyFigure) -> BodyFigureDto {
    let spheroid = figure.spheroid();
    BodyFigureDto {
        equatorial_radius_m: spheroid.equatorial_radius_m,
        polar_radius_m: spheroid.polar_radius_m,
        flattening: spheroid.flattening(),
        pole: figure.pole(),
        moment_of_inertia_factor: figure.moment_factor(),
        law: match figure.law() {
            FigureLaw::Sphere => FigureLawDto::Sphere,
            FigureLaw::Rotational => FigureLawDto::Rotational,
            FigureLaw::RotationalAndTidal => FigureLawDto::RotationalAndTidal,
            FigureLaw::Capped => FigureLawDto::Capped,
        },
        datum: match figure.datum() {
            Datum::SolidSurface => FigureDatumDto::SolidSurface,
            Datum::OneBar => FigureDatumDto::OneBar,
        },
    }
}

/// A body's photometry as the wire carries it.
#[must_use]
fn photometry(photometry: &BodyPhotometry) -> BodyPhotometryDto {
    let bands = |bands: Bands| BandsDto {
        b: bands.b,
        v: bands.v,
        r: bands.r,
    };
    BodyPhotometryDto {
        geometric_albedo: bands(photometry.geometric_albedo()),
        phase_template: phase_template(photometry.template()),
        phase_exponent: bands(photometry.exponents()),
        lunar_lambert_share: photometry.lunar_lambert_share(),
        bond_albedo: photometry.bond_albedo().value(),
        bond_ratio: photometry.bond_ratio(),
        provisional: photometry.provisional(),
    }
}

/// A phase-curve template, as the wire names it.
#[must_use]
fn phase_template(template: PhaseTemplate) -> PhaseTemplateDto {
    match template {
        PhaseTemplate::Moon => PhaseTemplateDto::Moon,
        PhaseTemplate::Mercury => PhaseTemplateDto::Mercury,
        PhaseTemplate::Mars => PhaseTemplateDto::Mars,
        PhaseTemplate::Venus => PhaseTemplateDto::Venus,
        PhaseTemplate::Earth => PhaseTemplateDto::Earth,
        PhaseTemplate::Jupiter => PhaseTemplateDto::Jupiter,
        PhaseTemplate::Saturn => PhaseTemplateDto::Saturn,
        PhaseTemplate::Uranus => PhaseTemplateDto::Uranus,
        PhaseTemplate::Neptune => PhaseTemplateDto::Neptune,
        PhaseTemplate::AirlessIce => PhaseTemplateDto::AirlessIce,
        PhaseTemplate::Snowball => PhaseTemplateDto::Snowball,
        PhaseTemplate::Magma => PhaseTemplateDto::Magma,
    }
}

/// A substance as the wire names it: its registry key, never its index (decision-composition
/// §1.1).
#[must_use]
fn key(substance: SubstanceId) -> String {
    substance.substance().key().as_str().to_owned()
}

/// A mixture of gases as the wire carries it, in the record's order.
#[must_use]
fn gas_shares(gases: &[(SubstanceId, f64)]) -> Vec<GasShareDto> {
    gases
        .iter()
        .map(|&(species, mole_fraction)| GasShareDto {
            species: key(species),
            mole_fraction,
        })
        .collect()
}

/// Areas by substance as the wire carries them, in the record's order.
#[must_use]
fn substance_areas(areas: &[(SubstanceId, f64)]) -> Vec<SubstanceAreaDto> {
    areas
        .iter()
        .map(|&(substance, area_fraction)| SubstanceAreaDto {
            substance: key(substance),
            area_fraction,
        })
        .collect()
}

/// A surface's published condensates as the wire carries them (P14.T51.a, sent from P14.T54.a):
/// in the record's order, each substance by its registry key, its mass always sent, its layer
/// absent where the record has none (the registry has not read its phase's density,
/// decision-pre-23-review item 12), and its trapped area absent while the record holds no cold
/// traps ([`Surface::cold_traps_modelled`], until P14.T54.b; decision-p14-t54a-water-consistency
/// §5.3).
#[must_use]
fn condensates(condensates: &Condensates) -> Vec<SurfaceCondensateDto> {
    let traps = Surface::cold_traps_modelled();
    condensates
        .entries()
        .iter()
        .map(|entry| SurfaceCondensateDto {
            substance: key(entry.substance()),
            phase: match entry.phase() {
                CondensatePhase::Solid => SubstancePhaseDto::Solid,
                CondensatePhase::Liquid => SubstancePhaseDto::Liquid,
                CondensatePhase::Supercritical => SubstancePhaseDto::Supercritical,
            },
            reservoir_kg_m2: entry.reservoir().value(),
            reservoir_gel_m: entry.reservoir_gel().map(Metres::value),
            area_fraction: entry.area_fraction(),
            trapped_area_fraction: traps.then_some(entry.trapped_area_fraction()),
            seasonal: entry.is_seasonal(),
        })
        .collect()
}

/// A surface's crust as the wire carries it (P14.T51.c, sent from P14.T54.a): its crusts and its
/// provinces by registry key, its lid, its redox class and the mantle's ΔIW that class bands
/// (P14.T50.a's inventory's, [`CrustComposition::mantle_delta_iw`]), and its melt area, each the
/// record's to the bit.
///
/// The record's thermal history and peak central temperature, which decided the crusts, are not
/// sent: the wire's crust, whose shape froze at generator version 23, has no member for them
/// (decision-bump-plan-after-22, "Contracts that freeze at 23").
#[must_use]
fn crust(crust: &CrustComposition) -> CrustCompositionDto {
    CrustCompositionDto {
        primary: crust.primary().map(key),
        secondary: crust.secondary().map(key),
        tertiary: crust.tertiary().map(key),
        bulk: crust.bulk().map(key),
        lid: match crust.lid() {
            LidCrust::Primary => CrustLidDto::Primary,
            LidCrust::Secondary => CrustLidDto::Secondary,
            LidCrust::Bulk => CrustLidDto::Bulk,
        },
        provinces: key(crust.provinces()),
        redox: match crust.redox() {
            RedoxClass::Reduced => RedoxClassDto::Reduced,
            RedoxClass::Intermediate => RedoxClassDto::Intermediate,
            RedoxClass::Oxidised => RedoxClassDto::Oxidised,
        },
        mantle_redox_iw: crust.mantle_delta_iw().value(),
        melt_area_fraction: crust.melt_area_fraction(),
    }
}

/// A surface's modifiers as the wire carries them (P14.T51.d with T51.j, wired by P14.T54.b.c):
/// the maturity, the weathering rate k, the organic cover and the resurfacing rate, each the
/// record's to the bit, and an Io's volcanic and an oxidised surface's weathering deposits in the
/// record's order, each substance by its registry key with its share of the surface.
///
/// The rate's two terms (the wind's and the micrometeoroids') and the ground that can take a cover
/// are not sent: the wire's type has no member for them (P14.T51.d's shape, final at 24's first
/// send).
#[must_use]
fn surface_modifiers(modifiers: &SurfaceModifiers) -> SurfaceModifiersDto {
    SurfaceModifiersDto {
        maturity: modifiers.maturity(),
        weathering_rate: modifiers.weathering_rate(),
        organic_cover: modifiers.organic_cover(),
        resurfacing_m_per_yr: modifiers.resurfacing_m_per_yr(),
        volcanic_deposits: modifiers
            .volcanic_deposits()
            .iter()
            .map(|deposit| SubstanceAreaDto {
                substance: key(deposit.substance()),
                area_fraction: deposit.area_fraction(),
            })
            .collect(),
        weathering_deposits: modifiers
            .weathering_deposits()
            .iter()
            .map(|deposit| SubstanceAreaDto {
                substance: key(deposit.substance()),
                area_fraction: deposit.area_fraction(),
            })
            .collect(),
    }
}

/// A member of the surface section that a later task fills, as the wire carries it (P14.T35.e):
/// absent while this generator version does not model it, and once it does, its value converted
/// by `value`, which is `None` for a member computed as none, sent `null`.
#[must_use]
fn later<T, U>(member: RecordModelled<&T>, value: impl FnOnce(&T) -> Option<U>) -> Modelled<U> {
    match member {
        RecordModelled::Ok(inner) => value(inner).map_or(Modelled::Null, Modelled::Value),
        RecordModelled::NotModelled => Modelled::NotModelled,
    }
}

/// A surface section's value as the wire carries it (P14.T35.e), every member of the record's
/// [`Surface`] in its unit and every substance by its registry key.
///
/// The members of later tasks are absent, as the record holds them not modelled; each task fills
/// its own here when it lands, as P14.T54.a fills the condensates and the crust, which is absent
/// only where the record holds none (a body whose passes took no lid), and P14.T54.b the vertical
/// structure ([`vertical_structure_member`]) and the aerosol inventory ([`aerosol_inventory`],
/// never `null`: the empty inventory for a surface with no air), both absent while the record
/// holds the air's sections off, below generator version 24. The melt's own vapour,
/// `silicate_vapour`, has no member in the record and is sent absent from every surface until
/// P14.T55.b.
#[must_use]
fn surface(surface: &Surface) -> BodySurfaceDto {
    let conditions = surface.conditions();
    let figures = surface.figures();
    let regime = surface.climate_regime();
    BodySurfaceDto {
        state: surface_state(surface.state()),
        material: match surface.material() {
            SurfaceMaterial::Rock => SurfaceMaterialDto::Rock,
            SurfaceMaterial::Ice => SurfaceMaterialDto::Ice,
        },
        mean_surface_temperature_k: conditions.mean_surface_temperature().value(),
        equator_pole_contrast_k: conditions.equator_pole_contrast_k(),
        day_night_contrast_k: conditions.day_night_contrast_k(),
        surface_pressure_pa: conditions.surface_pressure().value(),
        gases: gas_shares(conditions.gases()),
        mean_molar_mass_g_per_mol: conditions.mean_molar_mass_g_per_mol(),
        heat_capacity_over_r: conditions.heat_capacity_over_r(),
        stellar_flux_w_m2: conditions.stellar_flux().value(),
        radiation_class: radiation_class(conditions.radiation()),
        liquid_water: conditions.liquid_water(),
        climate_regime: climate_regime(*regime),
        ocean_fraction: figures.ocean_fraction(),
        ice_fraction: figures.ice_fraction(),
        cloud_fraction: figures.cloud_fraction(),
        surface_liquids: substance_areas(figures.surface_liquids()),
        surface_ices: substance_areas(figures.surface_ices()),
        sigma_h_m: figures.sigma_h().value(),
        continental_fraction: figures.continental_fraction(),
        heat_flow_w_m2: figures.heat_flow().value(),
        tectonic_regime: tectonic_regime(figures.tectonic_regime()),
        volcanism_level: figures.volcanism(),
        outgassing_earth: figures.outgassing(),
        magnetic_field_class: match figures.magnetic_field() {
            MagneticField::None => MagneticFieldClassDto::None,
            MagneticField::Weak => MagneticFieldClassDto::Weak,
            MagneticField::Strong => MagneticFieldClassDto::Strong,
        },
        surface_age_s: Seconds::from(figures.surface_age()).value(),
        subsurface_ocean: match figures.subsurface_ocean() {
            Subsurface::NotComputed => Modelled::NotModelled,
            Subsurface::NoOcean => Modelled::Null,
            Subsurface::Ocean(ocean) => Modelled::Value(subsurface_ocean(ocean)),
        },
        craters: crater_contract(figures.craters()),
        wet_epoch: figures.wet_epoch().map(wet_epoch),
        vertical_structure: vertical_structure_member(surface.vertical_structure()),
        aerosols: later(surface.aerosols(), |inventory| {
            Some(aerosol_inventory(inventory))
        }),
        condensates: Modelled::Value(condensates(surface.condensates())),
        crust: later(surface.crust(), |composition| Some(crust(composition))),
        modifiers: later(surface.modifiers(), |modifiers| {
            Some(surface_modifiers(modifiers))
        }),
        biosphere: later(surface.biosphere(), |biosphere| match *biosphere {}),
        // P14.T55.b's: the record holds no such member, so every server sends it absent until
        // then (P14.T35.f).
        silicate_vapour: Modelled::NotModelled,
    }
}

/// A surface state as the wire names it.
///
/// # Panics
///
/// For [`SurfaceState::GasEnvelope`], which a record's surface never holds: [`Surface`] refuses it,
/// since a gas-envelope body's surface section is not applicable (decision-p14-t35e-wire §2).
#[must_use]
fn surface_state(state: SurfaceState) -> SurfaceStateDto {
    match state {
        SurfaceState::MagmaOcean => SurfaceStateDto::MagmaOcean,
        SurfaceState::Airless => SurfaceStateDto::Airless,
        SurfaceState::RunawayGreenhouse => SurfaceStateDto::RunawayGreenhouse,
        SurfaceState::Temperate => SurfaceStateDto::Temperate,
        SurfaceState::Snowball => SurfaceStateDto::Snowball,
        SurfaceState::Tenuous => SurfaceStateDto::Tenuous,
        SurfaceState::GasEnvelope => {
            unreachable!("a record's surface is never in the gas-envelope state")
        }
    }
}

/// A radiation class as the wire names it.
#[must_use]
fn radiation_class(class: RadiationClass) -> RadiationClassDto {
    match class {
        RadiationClass::Dark => RadiationClassDto::Dark,
        RadiationClass::Quiet => RadiationClassDto::Quiet,
        RadiationClass::Active => RadiationClassDto::Active,
        RadiationClass::Intense => RadiationClassDto::Intense,
        RadiationClass::Extreme => RadiationClassDto::Extreme,
    }
}

/// A climate regime as the wire carries it, its condensable by key.
#[must_use]
fn climate_regime(regime: ClimateRegime) -> ClimateRegimeDto {
    ClimateRegimeDto {
        thermal: match regime.thermal() {
            ThermalRegime::AirlessLike => ThermalRegimeDto::AirlessLike,
            ThermalRegime::Transitional => ThermalRegimeDto::Transitional,
            ThermalRegime::Efficient => ThermalRegimeDto::Efficient,
        },
        forcing: match regime.forcing() {
            Forcing::Seasonal => ClimateForcingDto::Seasonal,
            Forcing::IceBelt => ClimateForcingDto::IceBelt,
            Forcing::SlowRotator => ClimateForcingDto::SlowRotator,
            Forcing::Locked => ClimateForcingDto::Locked,
        },
        condensable: regime.condensable().map(key),
        model: climate_model(regime.model()),
    }
}

/// A tectonic regime as the wire names it.
#[must_use]
fn tectonic_regime(regime: TectonicRegime) -> TectonicRegimeDto {
    match regime {
        TectonicRegime::MobileLid => TectonicRegimeDto::MobileLid,
        TectonicRegime::StagnantLid => TectonicRegimeDto::StagnantLid,
        TectonicRegime::HeatPipe => TectonicRegimeDto::HeatPipe,
        TectonicRegime::Episodic => TectonicRegimeDto::Episodic,
        TectonicRegime::IceShell => TectonicRegimeDto::IceShell,
    }
}

/// A record's vertical structure member as the wire carries it (P14.T24.e, wired by P14.T54.b):
/// absent while the record does not model it, `null` for a surface computed to hold no air, and
/// otherwise its structure ([`vertical_structure`]).
#[must_use]
fn vertical_structure_member(
    member: RecordModelled<Option<&VerticalStructure>>,
) -> Modelled<VerticalStructureDto> {
    match member {
        RecordModelled::Ok(Some(structure)) => Modelled::Value(vertical_structure(structure)),
        RecordModelled::Ok(None) => Modelled::Null,
        RecordModelled::NotModelled => Modelled::NotModelled,
    }
}

/// A surface air's vertical structure as the wire carries it: its α, its skin's temperature in K
/// and its adiabat's nodes from the ground to the tropopause, each pressure in Pa and temperature
/// in K. Its ground's temperature and pressure are the surface section's own
/// `mean_surface_temperature_k` and `surface_pressure_pa`, which its first node repeats.
///
/// # Panics
///
/// For [`VerticalStructure::Envelope`], which a record's surface never holds: the surface's
/// structure is its air's, a `RadiativeConvective` (`system::air::record_structure` builds no
/// other), and a gas envelope's profile (P14.T24.d) belongs to the envelope section,
/// P14.T54.b.i's.
#[must_use]
fn vertical_structure(structure: &VerticalStructure) -> VerticalStructureDto {
    match structure {
        VerticalStructure::RadiativeConvective(profile) => VerticalStructureDto {
            alpha: profile.lapse_factor(),
            skin_temperature_k: profile.skin_temperature().value(),
            adiabat: profile
                .nodes()
                .iter()
                .map(|node| ProfileNodeDto {
                    pressure_pa: node.pressure().value(),
                    temperature_k: node.temperature().value(),
                })
                .collect(),
        },
        VerticalStructure::Envelope(_) => {
            unreachable!("a record's surface never carries a gas envelope's profile")
        }
    }
}

/// A surface air's aerosol and absorber inventory as the wire carries it (P14.T24.c with T24.g,
/// wired by P14.T54.b.b; decision-p14-t54b-open §2): its modes ([`aerosol_mode`]), its decks
/// ([`condensate_deck`]), its absorbers ([`absorber`]) and its condensates not formed, each in the
/// record's order and each substance by its registry key. A surface with no air's, or a magma
/// ocean's, is every list empty.
#[must_use]
fn aerosol_inventory(inventory: &AerosolInventory) -> AerosolInventoryDto {
    let decks = inventory.decks();
    AerosolInventoryDto {
        modes: inventory
            .modes()
            .iter()
            .map(|mode| aerosol_mode(mode, decks))
            .collect(),
        decks: decks.iter().map(condensate_deck).collect(),
        absorbers: inventory.absorbers().iter().map(absorber).collect(),
        not_formed: inventory
            .not_formed()
            .iter()
            .map(|&(condensate, why)| NotFormedCondensateDto {
                condensate: key(condensate),
                reason: match why {
                    NotFormed::BulkGas => NotFormedReasonDto::BulkGas,
                    NotFormed::Displaced { by } => NotFormedReasonDto::Displaced {
                        by: by.as_str().to_owned(),
                    },
                    NotFormed::Supercritical => NotFormedReasonDto::Supercritical,
                },
            })
            .collect(),
    }
}

/// A condensed phase as an aerosol mode's or a deck's wire names it: `solid` or `liquid`.
#[must_use]
fn aerosol_phase(phase: CondensedPhase) -> SubstancePhaseDto {
    match phase {
        CondensedPhase::Solid => SubstancePhaseDto::Solid,
        CondensedPhase::Liquid => SubstancePhaseDto::Liquid,
    }
}

/// An aerosol mode of an inventory whose decks are `decks`, as the wire carries it
/// (decision-p14-t54b-open §2.2), each value the record's to the bit:
///
/// - its `deck` is its deck's condensate where `decks` holds a deck of it, and `null` for a mode
///   of no deck, a rule's (the dust, the haze, a sulphate layer), whose own material the sim names
///   as its deck: never that material, since an acid deck and a sulphate layer never share
///   `H2SO4` on one body (P14.T24.g's case is the body's);
/// - a member the sim does not model (its size, its column mass, its τ(550) with its basis, its
///   temperature) is absent, never `null` or 0, and `not_modelled` gives the one reason
///   ([`mode_not_modelled`]);
/// - its τ(550) is in-cloud, over its coverage, as the sim computes it before the coverage applies.
#[must_use]
fn aerosol_mode(mode: &AerosolMode, decks: &[AerosolDeck]) -> AerosolModeDto {
    let profile = mode.profile();
    let depth = mode.optical_depth().ok();
    AerosolModeDto {
        material: key(mode.material()),
        phase: aerosol_phase(mode.phase()),
        deck: decks
            .iter()
            .any(|deck| deck.condensate() == mode.deck())
            .then(|| key(mode.deck())),
        shape: match mode.shape() {
            ShapeClass::Sphere => AerosolShapeDto::Sphere,
            ShapeClass::NonSphericalMineral => AerosolShapeDto::NonSphericalMineral,
            ShapeClass::Crystal => AerosolShapeDto::Crystal,
            ShapeClass::Aggregate => AerosolShapeDto::Aggregate,
        },
        size_distribution: mode.size().ok().map(size_distribution),
        column_mass_kg_m2: mode.column_mass().map(KilogramsPerSquareMetre::value),
        tau_550: depth.as_ref().map(ModeDepth::value),
        tau_basis: depth.map(|depth| tau_basis(depth.basis())),
        base_pa: profile.base().value(),
        top_pa: profile.top().value(),
        f_sed: profile.exponent(),
        coverage: mode.coverage(),
        temperature_k: mode.temperature().map(Kelvin::value),
        not_modelled: mode_not_modelled(mode),
    }
}

/// A mode's particle sizes as the wire carries them: a distribution of the volume-to-area radius
/// by its family, or an aggregate's monomers with its prefactor.
#[must_use]
fn size_distribution(size: ParticleSize) -> SizeDistributionDto {
    match size {
        ParticleSize::Distribution(size) => {
            let effective_radius_m = size.effective_radius().value();
            let effective_variance = size.effective_variance();
            match size.kind() {
                SizeDistributionKind::Gamma => SizeDistributionDto::Gamma {
                    effective_radius_m,
                    effective_variance,
                },
                SizeDistributionKind::LogNormal => SizeDistributionDto::LogNormal {
                    effective_radius_m,
                    effective_variance,
                },
            }
        }
        ParticleSize::Aggregate(aggregate) => SizeDistributionDto::Aggregate {
            monomer_radius_m: aggregate.monomer_radius().value(),
            monomer_count: aggregate.monomers(),
            fractal_dimension: aggregate.fractal_dimension(),
            fractal_prefactor: aggregate.prefactor(),
        },
    }
}

/// What a mode's τ(550) rests on, as the wire names it: P14.T49.e's table on its basis, the
/// geometric limit, or a rule's own column.
#[must_use]
fn tau_basis(basis: DepthBasis) -> AerosolTauBasisDto {
    match basis {
        DepthBasis::Table(ExtinctionBasis::Sphere) => AerosolTauBasisDto::Sphere,
        DepthBasis::Table(ExtinctionBasis::PhaseTable) => AerosolTauBasisDto::PhaseTable,
        DepthBasis::Table(ExtinctionBasis::SpheresOutsidePhaseTable) => {
            AerosolTauBasisDto::SpheresOutsidePhaseTable
        }
        DepthBasis::Table(ExtinctionBasis::SphereFallback) => AerosolTauBasisDto::SphereFallback,
        DepthBasis::GeometricLimit => AerosolTauBasisDto::GeometricLimit,
        DepthBasis::Rule => AerosolTauBasisDto::Rule,
    }
}

/// Why a mode's absent members are not modelled, `None` where every one is (decision-p14-t54b-open
/// §2.2): one reason a mode, since the sim's members fail together. A τ(550) needs its size, so a
/// size's reason is the τ's too; a rule's column mass is its τ over its mass extinction, so the
/// optics' reason is the column's too; and an aggregate's column mass and temperature are absent
/// for want of an aggregate's mass extinction, its τ a rule's.
#[must_use]
fn mode_not_modelled(mode: &AerosolMode) -> Option<AerosolNotModelledDto> {
    if let Err(error) = mode.size() {
        return Some(size_not_modelled(error));
    }
    if let Err(error) = mode.optical_depth() {
        return Some(match error {
            ResolveOpticalDepthError::Size(error) => size_not_modelled(error),
            ResolveOpticalDepthError::Aerosol(error) => optics_not_modelled(error),
        });
    }
    (mode.column_mass().is_none() || mode.temperature().is_none())
        .then_some(AerosolNotModelledDto::NoMassExtinction)
}

/// Why a mode's sizes are not modelled, as the wire names it.
#[must_use]
fn size_not_modelled(error: SizeModeError) -> AerosolNotModelledDto {
    match error {
        SizeModeError::NoDensity {
            phase, temperature, ..
        } => AerosolNotModelledDto::NoGrainDensity {
            phase: aerosol_phase(phase),
            temperature_k: temperature.value(),
        },
        SizeModeError::NoCollisionConstants { .. } => AerosolNotModelledDto::NoCollisionConstants,
        SizeModeError::NoCondensate => AerosolNotModelledDto::NoCondensate,
        SizeModeError::Distribution(_) => AerosolNotModelledDto::NoSizeDistribution,
    }
}

/// Why a mode's optics are not modelled (P14.T49.e's refusal), as the wire names it: a grain
/// density it lacks, or optics the table does not hold (no index file in that phase and variant,
/// a material that is not an aerosol's, a size off the grid).
#[must_use]
fn optics_not_modelled(error: ResolveAerosolError) -> AerosolNotModelledDto {
    match error {
        ResolveAerosolError::NoDensity {
            phase, temperature, ..
        } => AerosolNotModelledDto::NoGrainDensity {
            phase: aerosol_phase(phase),
            temperature_k: temperature.value(),
        },
        ResolveAerosolError::NotModelled { .. }
        | ResolveAerosolError::NoFile { .. }
        | ResolveAerosolError::OffGrid { .. }
        | ResolveAerosolError::NonFiniteTemperature { .. } => {
            AerosolNotModelledDto::OpticsNotModelled
        }
    }
}

/// A condensate deck as the wire carries it, each value the record's to the bit.
#[must_use]
fn condensate_deck(deck: &AerosolDeck) -> CondensateDeckDto {
    CondensateDeckDto {
        condensate: key(deck.condensate()),
        phase: aerosol_phase(deck.phase()),
        base_pa: deck.base().value(),
        base_temperature_k: deck.base_temperature().value(),
        top_pa: deck.top().value(),
        vapour_mole_fraction_below: deck.vapour_below(),
        vapour_mole_fraction_above: deck.vapour_above(),
        f_sed: deck.sedimentation(),
        extrapolated: deck.extrapolated(),
    }
}

/// An absorber as the wire carries it: its column and layer, a slab's between its base and top,
/// and a profile's from its first node to its last, each node's mixing ratio over the profile's
/// peak, so that the peak's is 1 (the sim's nodes are relative to one another: ozone's are the
/// AFGL template's ppmv), which the column scales.
///
/// # Panics
///
/// Never: a profile holds at least two nodes, each of a positive mixing ratio (`Absorber::profile`).
#[must_use]
fn absorber(absorber: &Absorber) -> AbsorberDto {
    let (base_pa, top_pa, profile) = match absorber.layer() {
        AbsorberLayer::Slab { base, top } => (base.value(), top.value(), None),
        AbsorberLayer::Profile(nodes) => {
            let (first, last) = (
                nodes.first().expect("a profile holds at least two nodes"),
                nodes.last().expect("a profile holds at least two nodes"),
            );
            let peak = nodes
                .iter()
                .map(AbsorberNode::relative_mixing_ratio)
                .fold(0.0_f64, f64::max);
            let relative = nodes
                .iter()
                .map(|node| MixingRatioNodeDto {
                    pressure_pa: node.pressure().value(),
                    relative_mixing_ratio: node.relative_mixing_ratio() / peak,
                })
                .collect();
            (
                first.pressure().value(),
                last.pressure().value(),
                Some(relative),
            )
        }
    };
    AbsorberDto {
        species: key(absorber.species()),
        column_m2: absorber.column_per_m2(),
        base_pa,
        top_pa,
        profile,
    }
}

/// A subsurface ocean as the wire carries it.
#[must_use]
fn subsurface_ocean(ocean: &SubsurfaceOcean) -> SubsurfaceOceanDto {
    SubsurfaceOceanDto {
        shell_thickness_m: ocean.shell_thickness().value(),
        thickness_m: ocean.thickness().value(),
        floor: match ocean.floor() {
            OceanFloor::Rock => OceanFloorDto::Rock,
            OceanFloor::IceIii => OceanFloorDto::IceIii,
            OceanFloor::IceV => OceanFloorDto::IceV,
            OceanFloor::IceVi => OceanFloorDto::IceVi,
            OceanFloor::IceVii => OceanFloorDto::IceVii,
        },
        liquid: key(ocean.liquid()),
        solute_mass_fraction: ocean.solute_mass_fraction(),
        base_temperature_k: ocean.base_temperature().value(),
    }
}

/// A crater contract as the wire carries it; its gravity is the bulk section's, not repeated.
#[must_use]
fn crater_contract(craters: &CraterParams) -> CraterContractDto {
    let target = craters.target();
    CraterContractDto {
        lunar_equivalent_n_1km_per_km2: craters.n_1km().value(),
        screening: match craters.screening() {
            Screening::None => CraterScreeningDto::None,
            Screening::Atmosphere { column_mass } => CraterScreeningDto::Atmosphere {
                column_mass_kg_m2: column_mass.value(),
            },
            Screening::Cutoff { diameter } => CraterScreeningDto::Cutoff {
                diameter_m: diameter.value(),
            },
        },
        projectile_density_kg_m3: craters.projectile_density().value(),
        k_target: craters.k_target(),
        target_class: match target.class {
            TargetClass::NonPorous => CraterTargetClassDto::NonPorous,
            TargetClass::Porous => CraterTargetClassDto::Porous,
        },
        target_density_kg_m3: target.density.value(),
        system_age_s: Seconds::from(craters.system_age()).value(),
        impact_velocity_m_s: craters.impact_velocity().map(MetresPerSecond::value),
    }
}

/// A wet epoch as the wire carries it, its times in seconds before the present.
#[must_use]
fn wet_epoch(epoch: &WetEpoch) -> WetEpochDto {
    WetEpochDto {
        liquid: key(epoch.liquid()),
        start_ago_s: Seconds::from(epoch.start()).value(),
        end_ago_s: Seconds::from(epoch.end()).value(),
        effective_flow_yr: epoch.effective_flow().value(),
        runoff_m_per_yr: epoch.runoff_m_per_year(),
        paleo_inventory_kg_m2: epoch.paleo_inventory().value(),
    }
}

/// An envelope section's value as the wire carries it: none yet, since the record's
/// [`Envelope`] has no value until P14.T24.d, which fills it here.
#[must_use]
fn envelope(envelope: Envelope) -> BodyEnvelopeDto {
    match envelope {}
}

/// A hooks section's value: the body's detail seed (the rendering plans' R09.T1.b), which the
/// client's local synthesis draws from. The surface seed is not in the record, so it cannot reach
/// the wire from here.
#[must_use]
fn hooks(hooks: &Hooks) -> BodyHooksDto {
    BodyHooksDto {
        detail_seed: SectionDto::Ok(DetailSeedHex::from_u64(hooks.detail_seed().get())),
    }
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

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::params::GalaxyParams;
    use hyperion_sim::substance;
    use hyperion_sim::units::Dex;
    use serde_json::json;

    use super::*;

    /// The seed of plan 14's golden systems (P14.T32), the sim's `planetary_golden.rs`'s.
    const GOLDEN_SEED: u64 = 0x5eed_0000_0014_0032;

    /// The fifteen golden systems of the sim's `planetary_golden.rs` (`GOLDEN_SYSTEMS`), in its
    /// order: the resonant chain, the hot Jupiter, the Solar-like star, the eccentric giant, the
    /// halo star, the close and wide binaries, the hierarchical triple, the subgiant, the red
    /// giant, the fallback black hole, the three fillers and the T Tauri star.
    const GOLDEN_SYSTEMS: [u64; 15] = [
        0x0200_2b2c_a000_0000,
        0x41fc_ecba_0000_0006,
        0x4200_6cba_0000_0009,
        0x4200_acaa_0000_000a,
        0x0202_5b2b_e000_0001,
        0x4200_2cb2_0000_0009,
        0x41ff_ecae_0000_0004,
        0x4200_2cb2_0000_0003,
        0x4200_6c9a_0000_0004,
        0x4200_ecd2_0000_0003,
        0x8200_b2e0_0000_000a,
        0x01ff_fb2c_6000_0000,
        0x2200_1659_8000_0000,
        0x4200_2cb2_0000_0000,
        0x41fe_2c96_0000_000c,
    ];

    /// The golden systems' galaxy, the Milky Way fixture at [`GOLDEN_SEED`].
    static GOLDEN_GALAXY: LazyLock<Galaxy> = LazyLock::new(|| {
        Galaxy::from_params(Seed::new(GOLDEN_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    });

    /// Every golden system's records at the epoch at the `full` level, each beside its wire form.
    static GOLDEN_RECORDS: LazyLock<Vec<(BodyRecord, BodyRecordDto)>> = LazyLock::new(|| {
        let galaxy = &*GOLDEN_GALAXY;
        GOLDEN_SYSTEMS
            .iter()
            .flat_map(|&raw| {
                let id = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
                let ctx =
                    SystemContext::for_system(galaxy, id).expect("a pinned ID names a system");
                let planets = hyperion_sim::planetary::generate(galaxy.seed(), &ctx);
                let snapshot = planets
                    .snapshot_at(&ctx, UniverseTime::EPOCH)
                    .degrade(DetailLevel::Full);
                snapshot
                    .bodies()
                    .iter()
                    .map(|record| (record.clone(), body_record(record)))
                    .collect::<Vec<_>>()
            })
            .collect()
    });

    /// The golden records of present planets with a bulk section, with their class.
    fn present_planets()
    -> impl Iterator<Item = (PlanetClass, &'static BodyRecord, &'static BodyRecordDto)> {
        GOLDEN_RECORDS.iter().filter_map(|(sim, wire)| {
            let Section::Ok(bulk) = sim.bulk() else {
                return None;
            };
            (sim.identity().kind() == BodyKind::Planet
                && sim.identity().state() == BodyState::Present)
                .then_some((bulk.class(), sim, wire))
        })
    }

    /// The surface sections that are `ok`, the record's and the wire's, with the wire's record.
    fn ok_surfaces() -> impl Iterator<
        Item = (
            &'static Surface,
            &'static BodySurfaceDto,
            &'static BodyRecordDto,
        ),
    > {
        GOLDEN_RECORDS
            .iter()
            .filter_map(|(sim, wire)| match (sim.surface(), &wire.surface) {
                (Section::Ok(sim), SectionDto::Ok(dto)) => Some((sim, dto, wire)),
                _ => None,
            })
    }

    /// P14.T35.e: a present planet has exactly one of its surface and its envelope other than
    /// `not_applicable`; a giant's and a sub-Neptune's surface is not applicable and its envelope
    /// not modelled until P14.T24.d, as, from generator version 24, a solid planet's under a
    /// supercritical envelope (P14.T53.b part 2); and every record carries the envelope.
    #[test]
    fn every_present_planet_has_exactly_one_of_a_surface_and_an_envelope() {
        let (mut solid, mut sub_neptunes, mut giants) = (0, 0, 0);
        for (class, sim, wire) in present_planets() {
            let envelope = wire
                .envelope
                .as_ref()
                .expect("the server always sends the envelope");
            let applies = [
                wire.surface != SectionDto::NotApplicable,
                *envelope != SectionDto::NotApplicable,
            ];
            assert_eq!(
                applies.iter().filter(|&&applies| applies).count(),
                1,
                "{}",
                wire.id
            );
            // From generator version 24 a solid class under a supercritical envelope (P14.T53.b
            // part 2) has its surface not applicable and its envelope not modelled, as a gas
            // envelope's: the test that decides it is the simulation's own, so the tag pair alone
            // stands for it here.
            let hidden = class.has_solid_surface()
                && wire.surface == SectionDto::NotApplicable
                && *envelope == SectionDto::NotModelled;
            assert!(
                !hidden || hyperion_sim::GENERATOR_VERSION.get() >= 24,
                "{}",
                wire.id
            );
            if hidden {
                assert_eq!(sim.envelope(), &Section::NotModelled, "{}", wire.id);
            } else if class.has_solid_surface() {
                assert!(matches!(wire.surface, SectionDto::Ok(_)), "{}", wire.id);
                solid += 1;
            } else {
                assert_eq!(wire.surface, SectionDto::NotApplicable, "{}", wire.id);
                assert_eq!(*envelope, SectionDto::NotModelled, "{}", wire.id);
                assert_eq!(sim.envelope(), &Section::NotModelled, "{}", wire.id);
                match class {
                    PlanetClass::SubNeptune => sub_neptunes += 1,
                    PlanetClass::IceGiant | PlanetClass::GasGiant => giants += 1,
                    PlanetClass::Rocky | PlanetClass::Icy => {
                        panic!("{}: a solid class without a solid surface", wire.id);
                    }
                }
            }
        }
        assert!(
            GOLDEN_RECORDS
                .iter()
                .all(|(_, wire)| wire.envelope.is_some())
        );
        assert!(
            solid > 10 && sub_neptunes > 0 && giants > 10,
            "{solid} solid, {sub_neptunes} sub-Neptunes, {giants} giants"
        );
    }

    /// P14.T35.e: T48.e's stopgap is gone. Every `ok` surface of the record is `ok` on the wire
    /// with the record's values, and every other state is the record's.
    #[test]
    fn every_surface_is_sent_in_the_record_s_state_with_its_values() {
        for (sim, wire) in GOLDEN_RECORDS.iter() {
            let same = matches!(
                (sim.surface(), &wire.surface),
                (Section::Ok(_), SectionDto::Ok(_))
                    | (Section::NotResolved, SectionDto::NotResolved)
                    | (Section::NotModelled, SectionDto::NotModelled)
                    | (Section::NotApplicable, SectionDto::NotApplicable)
            );
            assert!(
                same,
                "{}: the wire's surface is not in the record's state",
                wire.id
            );
        }
        let mut surfaces = 0;
        for (sim, dto, wire) in ok_surfaces() {
            let conditions = sim.conditions();
            let figures = sim.figures();
            let id = wire.id.as_str();
            assert_eq!(
                (
                    dto.mean_surface_temperature_k.to_bits(),
                    dto.surface_pressure_pa.to_bits(),
                    dto.ocean_fraction.to_bits(),
                    dto.heat_flow_w_m2.to_bits(),
                    dto.craters.k_target.to_bits(),
                ),
                (
                    conditions.mean_surface_temperature().value().to_bits(),
                    conditions.surface_pressure().value().to_bits(),
                    figures.ocean_fraction().to_bits(),
                    figures.heat_flow().value().to_bits(),
                    figures.craters().k_target().to_bits(),
                ),
                "{id}"
            );
            assert_eq!(dto.gases.len(), conditions.gases().len(), "{id}");
            assert_eq!(
                dto.wet_epoch.is_some(),
                figures.wet_epoch().is_some(),
                "{id}"
            );
            surfaces += 1;
        }
        assert!(surfaces > 50, "{surfaces} surfaces");
    }

    /// P14.T35.e: the members of later tasks are absent, as the record holds them, with
    /// P14.T35.f's `silicate_vapour`, which the record does not hold, and a subsurface ocean is
    /// computed for a regular moon alone, an object or `null`, until P14.T51.g.b's bump, and for
    /// every solid surface from it. From P14.T51.g.a's bump (23 → 24) a regular moon's is absent
    /// where its record's is not computed, its shell's base lying at or beyond ice V's melting
    /// pressure (liquid under high-pressure ice, P14.T55.k's), and none is below it; from
    /// P14.T51.g.b's, any body's is absent where its column keeps liquid only under high-pressure
    /// ice or its base lies in ice II. An ocean rests on rock or, from those bumps, on ice III or
    /// ice V, never on ice VI or VII while it is at most 273.16 K. The condensates
    /// and the crust are sent from P14.T54.a (`every_surface_sends_its_record_s_condensates`,
    /// `every_solid_surface_sends_its_record_s_crust`). The vertical structure and the aerosol
    /// inventory are not modelled on every golden body's record and absent on the wire while the
    /// sim holds the air's sections off the record, below generator version 24
    /// (`AIR_SECTIONS_VERSION`, whose tripwire `the_air_sections_are_held_only_until_the_24_bump`
    /// this is the wire's half of; at the bump, P14.T54.b.l, both are present here, the structure
    /// `null` and the inventory the empty object on an airless surface;
    /// `a_vertical_structure_is_sent_as_its_alpha_skin_and_adiabat` and
    /// `an_aerosol_inventory_is_sent_as_the_record_holds_it` test their conversions). So are the
    /// modifiers, held behind the same hold (P14.T54.b.c; at the bump present on every surface
    /// whose record models them; `a_surface_s_modifiers_are_sent_as_the_record_holds_them` tests
    /// their conversion).
    #[test]
    fn the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_is_computed() {
        let (mut moons, mut absent) = (0, Vec::new());
        for (sim, dto, wire) in ok_surfaces() {
            assert_eq!(
                (sim.vertical_structure(), sim.aerosols()),
                (RecordModelled::NotModelled, RecordModelled::NotModelled),
                "{}: the air's sections are held",
                wire.id
            );
            assert_eq!(
                sim.modifiers(),
                RecordModelled::NotModelled,
                "{}: the modifiers are held with the air's sections",
                wire.id
            );
            assert_eq!(
                [
                    dto.vertical_structure.is_not_modelled(),
                    dto.aerosols.is_not_modelled(),
                    dto.modifiers.is_not_modelled(),
                    dto.biosphere.is_not_modelled(),
                    dto.silicate_vapour.is_not_modelled(),
                ],
                [true; 5],
                "{}: the structure, aerosols, modifiers, biosphere and silicate vapour",
                wire.id
            );
            let regular = wire.kind
                == BodyKindDto::Moon {
                    origin: MoonOriginDto::Regular,
                };
            let not_computed = matches!(sim.figures().subsurface_ocean(), Subsurface::NotComputed);
            // From P14.T51.g.b's bump every solid surface's ocean is computed.
            let every_body =
                hyperion_sim::GENERATOR_VERSION >= hyperion_sim::GeneratorVersion::new(24);
            assert_eq!(
                dto.subsurface_ocean.is_not_modelled(),
                (!regular && !every_body) || not_computed,
                "{}",
                wire.id
            );
            if regular && not_computed {
                absent.push(wire.id.to_string());
            }
            if let Modelled::Value(ocean) = &dto.subsurface_ocean {
                assert!(
                    ocean.thickness_m > 0.0
                        && ocean.shell_thickness_m >= 0.0
                        && matches!(
                            ocean.floor,
                            OceanFloorDto::Rock | OceanFloorDto::IceIii | OceanFloorDto::IceV
                        ),
                    "{}: {ocean:?}",
                    wire.id
                );
            }
            moons += usize::from(regular);
        }
        assert!(moons > 0, "no regular moon's ocean was checked");
        // Version 23 holds P14.T51.g.a's floor: every regular moon's ocean is computed.
        if hyperion_sim::GENERATOR_VERSION < hyperion_sim::GeneratorVersion::new(24) {
            assert_eq!(absent, Vec::<String>::new(), "absent at 23");
        }
    }

    /// P14.T54.b.c: a surface's modifiers are sent as the record holds them, each scalar to the bit
    /// and each deposit by its registry key with its share, in the record's order: an Io-like
    /// moon's SO₂ frost and sulphur, with no weathering deposit, and a Mars-like world's `ferric`,
    /// with no volcanic one, each derived through the sim's public API from inputs by hand (Io's
    /// 2.5 W m⁻² at 110 K inside a Jupiter's magnetopause; Mars's 636 Pa of CO₂, N₂ and Ar over
    /// `basalt_fe`, dry for 3.5 Gyr), since no record carries them while the sim holds them off it
    /// (below generator version 24).
    #[test]
    fn a_surface_s_modifiers_are_sent_as_the_record_holds_them() {
        let none = Condensates::default();
        let areas = |dto: &[SubstanceAreaDto]| {
            dto.iter()
                .map(|area| (area.substance.clone(), area.area_fraction.to_bits()))
                .collect::<Vec<_>>()
        };
        let held = |deposits: &mut dyn Iterator<Item = (SubstanceId, f64)>| {
            deposits
                .map(|(id, area)| (key(id), area.to_bits()))
                .collect::<Vec<_>>()
        };
        let expected: [(&[&str], &[&str]); 2] = [(&["SO2", "S8"], &[]), (&[], &["ferric"])];
        for ((name, inputs), (volcanic, weathering)) in
            io_and_mars_inputs(&none).into_iter().zip(expected)
        {
            let modifiers = SurfaceModifiers::derive(&inputs).expect("the inputs are in range");
            let dto = surface_modifiers(&modifiers);
            for (sent, kept, member) in [
                (dto.maturity, modifiers.maturity(), "maturity"),
                (dto.weathering_rate, modifiers.weathering_rate(), "rate"),
                (dto.organic_cover, modifiers.organic_cover(), "cover"),
                (
                    dto.resurfacing_m_per_yr,
                    modifiers.resurfacing_m_per_yr(),
                    "resurfacing",
                ),
            ] {
                assert_eq!(sent.to_bits(), kept.to_bits(), "{name}: {member}");
            }
            let mut erupted = modifiers
                .volcanic_deposits()
                .iter()
                .map(|d| (d.substance(), d.area_fraction()));
            assert_eq!(areas(&dto.volcanic_deposits), held(&mut erupted), "{name}");
            let mut oxidised = modifiers
                .weathering_deposits()
                .iter()
                .map(|d| (d.substance(), d.area_fraction()));
            assert_eq!(
                areas(&dto.weathering_deposits),
                held(&mut oxidised),
                "{name}"
            );
            let keys = |dto: &[SubstanceAreaDto]| {
                dto.iter()
                    .map(|area| area.substance.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(keys(&dto.volcanic_deposits), volcanic, "{name}");
            assert_eq!(keys(&dto.weathering_deposits), weathering, "{name}");
            eprintln!(
                "{name}: m {:.4}, k {:.4e}, cover {:.3e}, resurfacing {:.3e} m/yr, volcanic {:?}, \
                 weathering {:?}",
                dto.maturity,
                dto.weathering_rate,
                dto.organic_cover,
                dto.resurfacing_m_per_yr,
                dto.volcanic_deposits,
                dto.weathering_deposits
            );
        }
    }

    /// The Io-like and Mars-like modifiers' inputs of
    /// `a_surface_s_modifiers_are_sent_as_the_record_holds_them`, by hand, with no condensates
    /// (`none`): the figures quoted there, the Sun as the host.
    fn io_and_mars_inputs(
        none: &Condensates,
    ) -> [(
        &'static str,
        hyperion_sim::planetary::hooks::ModifierInputs<'_>,
    ); 2] {
        use hyperion_sim::planetary::derive::atmosphere::Gas;
        use hyperion_sim::planetary::hooks::modifiers::{
            HeliocentricOrbit, ModifierHost, ModifierInputs, PlanetMagnetosphere, Primary,
            WaterHistory,
        };
        use hyperion_sim::planetary::testing::partial_pressures;
        use hyperion_sim::units::consts::METRES_PER_AU;
        use hyperion_sim::units::{
            Gigayears, MetresPerSecondSquared, Pascals, WattsPerSquareMetre, Years,
        };

        let row = |key: &str| substance::by_key(key).expect("a registry row");
        let au = |a: f64| Metres::new(a * METRES_PER_AU);
        let io = ModifierInputs {
            host: ModifierHost::sun(),
            orbit: HeliocentricOrbit {
                semi_major_axis: au(5.2038),
                eccentricity: 0.0489,
            },
            primary: Primary::Planet(PlanetMagnetosphere {
                radius: Metres::new(6.9911e7),
                field: MagneticField::Strong,
                semi_major_axis: Metres::new(4.217e8),
            }),
            gravity: MetresPerSecondSquared::new(1.796),
            surface_pressure: Pascals::ZERO,
            field: MagneticField::None,
            surface_age: Years::new(4.4e3),
            resurfacing_time: Years::new(4.4e3),
            heat_flow: WattsPerSquareMetre::new(2.5),
            surface_temperature: Kelvin::new(110.0),
            mantle_sulphur: Kilograms::new(1.0e21),
            condensates: none,
            lid: row("basalt"),
            gases: partial_pressures(|_| Pascals::ZERO),
            water: WaterHistory::NeverWet,
        };
        let air = [
            (Gas::CarbonDioxide, 614.0),
            (Gas::Nitrogen, 10.6),
            (Gas::Argon, 11.4),
        ];
        let mars = ModifierInputs {
            orbit: HeliocentricOrbit {
                semi_major_axis: au(1.523_679),
                eccentricity: 0.0934,
            },
            primary: Primary::Star,
            gravity: MetresPerSecondSquared::new(3.73),
            surface_pressure: Pascals::new(636.0),
            surface_age: Years::new(3.9e9),
            resurfacing_time: Years::new(4.2e9),
            heat_flow: WattsPerSquareMetre::new(0.022),
            surface_temperature: Kelvin::new(210.0),
            lid: row("basalt_fe"),
            gases: partial_pressures(|gas| {
                air.iter()
                    .find(|&&(listed, _)| listed == gas)
                    .map_or(Pascals::ZERO, |&(_, pa)| Pascals::new(pa))
            }),
            water: WaterHistory::DrySince(Gigayears::new(3.5)),
            ..io
        };
        [("Io-like", io), ("Mars-like", mars)]
    }

    /// P14.T54.b: a vertical structure is sent as its α, its skin's temperature and its adiabat's
    /// nodes, each to the bit and in the record's order, the ground first; a member computed as
    /// none is `null`, and one not modelled absent. The structure is an Earth's by hand, built
    /// through the sim's public API, since no record carries one while the sim holds the air's
    /// sections off it (below generator version 24).
    #[test]
    fn a_vertical_structure_is_sent_as_its_alpha_skin_and_adiabat() {
        use hyperion_sim::planetary::derive::atmosphere::Gas;
        use hyperion_sim::planetary::hooks::structure::{RadiativeConvective, StructureInputs};
        use hyperion_sim::units::Pascals;

        let gases = [
            (Gas::Nitrogen.substance(), 0.77),
            (Gas::Oxygen.substance(), 0.21),
            (Gas::Water.substance(), 0.01),
            (Gas::Argon.substance(), 0.01),
        ];
        let profile = RadiativeConvective::derive(&StructureInputs {
            surface_temperature: Kelvin::new(288.0),
            surface_pressure: Pascals::new(1.0e5),
            gases: &gases,
            condensable: Some(Gas::Water.substance()),
            skin_temperature: Kelvin::new(214.0),
        })
        .unwrap();
        let structure = VerticalStructure::RadiativeConvective(profile.clone());
        let dto = vertical_structure(&structure);
        assert_eq!(dto.alpha.to_bits(), profile.lapse_factor().to_bits());
        assert_eq!(
            dto.skin_temperature_k.to_bits(),
            profile.skin_temperature().value().to_bits()
        );
        assert!(profile.nodes().len() > 2, "{:?}", profile.nodes());
        assert_eq!(dto.adiabat.len(), profile.nodes().len());
        for (sent, node) in dto.adiabat.iter().zip(profile.nodes()) {
            assert_eq!(
                (sent.pressure_pa.to_bits(), sent.temperature_k.to_bits()),
                (
                    node.pressure().value().to_bits(),
                    node.temperature().value().to_bits()
                )
            );
        }
        assert_eq!(
            (
                dto.adiabat[0].pressure_pa.to_bits(),
                dto.adiabat[0].temperature_k.to_bits()
            ),
            (1.0e5_f64.to_bits(), 288.0_f64.to_bits())
        );
        assert_eq!(
            vertical_structure_member(RecordModelled::Ok(Some(&structure))),
            Modelled::Value(dto)
        );
        assert_eq!(
            vertical_structure_member(RecordModelled::Ok(None)),
            Modelled::Null
        );
        assert_eq!(
            vertical_structure_member(RecordModelled::NotModelled),
            Modelled::NotModelled
        );
    }

    /// An air by hand, as the record's inventory reads one (P14.T54.b.b), built through the sim's
    /// public API since no record carries an inventory while the sim holds the air's sections off
    /// it: its gases by registry key, surface temperature and pressure, skin, condensable,
    /// gravity, state, outgassing and liquid-water flag, its decks covering two-thirds of the sky
    /// and its liquids half the ground, under a Sun of 5,772 K.
    struct HandAir {
        gases: &'static [(&'static str, f64)],
        surface_temperature_k: f64,
        surface_pressure_pa: f64,
        skin_temperature_k: f64,
        condensable: Option<&'static str>,
        gravity_m_s2: f64,
        state: SurfaceState,
        outgassing: f64,
        liquid_water: bool,
    }

    impl HandAir {
        /// Its inventory, `AerosolInventory::derive`'s.
        fn inventory(&self) -> AerosolInventory {
            use hyperion_sim::planetary::hooks::aerosols::AerosolInputs;
            use hyperion_sim::planetary::hooks::structure::{RadiativeConvective, StructureInputs};
            use hyperion_sim::planetary::hooks::sulphur::RunawayDeck;
            use hyperion_sim::units::{MetresPerSecondSquared, Pascals};

            let row = |key: &str| substance::by_key(key).expect("a registry row");
            let gases: Vec<(SubstanceId, f64)> =
                self.gases.iter().map(|&(key, x)| (row(key), x)).collect();
            let structure = RadiativeConvective::derive(&StructureInputs {
                surface_temperature: Kelvin::new(self.surface_temperature_k),
                surface_pressure: Pascals::new(self.surface_pressure_pa),
                gases: &gases,
                condensable: self.condensable.map(row),
                skin_temperature: Kelvin::new(self.skin_temperature_k),
            })
            .expect("an air");
            let molar_mass = gases.iter().fold(0.0, |sum, &(id, x)| {
                sum + x * id.substance().molar_mass_g_per_mol().expect("a molar mass")
            });
            AerosolInventory::derive(&AerosolInputs {
                state: self.state,
                surface_pressure: Pascals::new(self.surface_pressure_pa),
                gases: &gases,
                mean_molar_mass_g_per_mol: molar_mass,
                gravity: MetresPerSecondSquared::new(self.gravity_m_s2),
                structure: &structure,
                cloud_fraction: 0.67,
                star_temperature: Some(Kelvin::new(5_772.0)),
                surface_liquid_cover: 0.5,
                outgassing: self.outgassing,
                liquid_water: self.liquid_water,
                runaway_deck: RunawayDeck::Kept,
            })
            .expect("an inventory")
        }
    }

    /// An oxygenated Earth by hand: 1 bar of nitrogen and oxygen with 1.3% water at 288 K under a
    /// 214 K skin, outgassing at Earth's rate over liquid water (a water deck, P14.T24.g's sulphate
    /// layer, its SO₂ and ozone).
    const EARTH_AIR: HandAir = HandAir {
        gases: &[("N2", 0.776), ("O2", 0.21), ("H2O", 0.013), ("Ar", 0.001)],
        surface_temperature_k: 288.0,
        surface_pressure_pa: 1.0e5,
        skin_temperature_k: 214.0,
        condensable: Some("H2O"),
        gravity_m_s2: 9.81,
        state: SurfaceState::Temperate,
        outgassing: 1.0,
        liquid_water: true,
    };

    /// A Venus by hand: 92 bar of carbon dioxide with 3.5% nitrogen at 735 K under a 190 K skin,
    /// a runaway greenhouse at P14.T24.g's Venus outgassing (its acid deck of four modes).
    const VENUS_AIR: HandAir = HandAir {
        gases: &[("CO2", 0.965), ("N2", 0.035)],
        surface_temperature_k: 735.0,
        surface_pressure_pa: 92.0e5,
        skin_temperature_k: 190.0,
        condensable: None,
        gravity_m_s2: 8.87,
        state: SurfaceState::RunawayGreenhouse,
        outgassing: 0.363_47,
        liquid_water: false,
    };

    /// A Titan by hand: Huygens's 1.467 bar of nitrogen with 5.65% methane at 93.65 K under a 70 K
    /// skin (its methane haze, an aggregate).
    const TITAN_AIR: HandAir = HandAir {
        gases: &[("N2", 0.9435), ("CH4", 0.0565)],
        surface_temperature_k: 93.65,
        surface_pressure_pa: 1.467e5,
        skin_temperature_k: 70.0,
        condensable: Some("CH4"),
        gravity_m_s2: 1.352,
        state: SurfaceState::Snowball,
        outgassing: 0.0,
        liquid_water: false,
    };

    /// What [`assert_modes_sent`] counts: the sulphate layers, the acid deck's modes and the
    /// hazes it met.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct ModesSent {
        layers: usize,
        acid_modes: usize,
        hazes: usize,
    }

    /// Asserts that the wire's modes `sent` are `inventory`'s, of the air `name`, each value to
    /// the bit, `deck` by the inventory's decks, and `not_modelled` present exactly where a member
    /// is absent; counted into `counts`.
    fn assert_modes_sent(
        name: &str,
        inventory: &AerosolInventory,
        sent: &[AerosolModeDto],
        counts: &mut ModesSent,
    ) {
        let acid = substance::by_key("H2SO4").expect("a registry row");
        let bits = |value: Option<f64>| value.map(f64::to_bits);
        assert_eq!(sent.len(), inventory.modes().len(), "{name}");
        for (sent, mode) in sent.iter().zip(inventory.modes()) {
            assert_eq!(sent.material, key(mode.material()), "{name}");
            let listed = inventory
                .decks()
                .iter()
                .any(|deck| deck.condensate() == mode.deck());
            assert_eq!(sent.deck, listed.then(|| key(mode.deck())), "{name}");
            assert_eq!(
                bits(sent.column_mass_kg_m2),
                bits(mode.column_mass().map(KilogramsPerSquareMetre::value)),
                "{name}"
            );
            assert_eq!(
                bits(sent.tau_550),
                bits(mode.optical_depth().ok().as_ref().map(ModeDepth::value)),
                "{name}"
            );
            assert_eq!(sent.tau_550.is_some(), sent.tau_basis.is_some(), "{name}");
            assert_eq!(
                bits(sent.temperature_k),
                bits(mode.temperature().map(Kelvin::value)),
                "{name}"
            );
            assert_eq!(sent.coverage.to_bits(), mode.coverage().to_bits(), "{name}");
            let profile = mode.profile();
            assert_eq!(
                (
                    sent.base_pa.to_bits(),
                    sent.top_pa.to_bits(),
                    sent.f_sed.to_bits()
                ),
                (
                    profile.base().value().to_bits(),
                    profile.top().value().to_bits(),
                    profile.exponent().to_bits()
                ),
                "{name}"
            );
            let all = sent.size_distribution.is_some()
                && sent.column_mass_kg_m2.is_some()
                && sent.tau_550.is_some()
                && sent.temperature_k.is_some();
            assert_eq!(sent.not_modelled.is_none(), all, "{name}: {sent:?}");
            if mode.deck() == acid {
                if listed {
                    counts.acid_modes += 1;
                } else {
                    counts.layers += 1;
                }
            }
            if sent.shape == AerosolShapeDto::Aggregate {
                assert_eq!(
                    (sent.deck.as_ref(), sent.tau_basis, sent.not_modelled),
                    (
                        None,
                        Some(AerosolTauBasisDto::Rule),
                        Some(AerosolNotModelledDto::NoMassExtinction)
                    ),
                    "{name}"
                );
                counts.hazes += 1;
            }
        }
    }

    /// Asserts that the wire's absorbers `sent` are `absorbers`, of the air `name`: each column to
    /// the bit, a slab's layer, and a profile's nodes at their pressures, each mixing ratio over
    /// the peak's, the peak's 1, the layer from the first node to the last.
    fn assert_absorbers_sent(name: &str, absorbers: &[Absorber], sent: &[AbsorberDto]) {
        assert_eq!(sent.len(), absorbers.len(), "{name}");
        for (sent, absorber) in sent.iter().zip(absorbers) {
            assert_eq!(sent.species, key(absorber.species()), "{name}");
            assert_eq!(sent.column_m2.to_bits(), absorber.column_per_m2().to_bits());
            match (absorber.layer(), &sent.profile) {
                (AbsorberLayer::Slab { base, top }, None) => assert_eq!(
                    (sent.base_pa.to_bits(), sent.top_pa.to_bits()),
                    (base.value().to_bits(), top.value().to_bits())
                ),
                (AbsorberLayer::Profile(nodes), Some(profile)) => {
                    assert_eq!(profile.len(), nodes.len(), "{name}");
                    let peak = profile
                        .iter()
                        .map(|node| node.relative_mixing_ratio)
                        .fold(0.0_f64, f64::max);
                    assert_eq!(peak.to_bits(), 1.0_f64.to_bits(), "{name}");
                    for (node, held) in profile.iter().zip(nodes) {
                        assert_eq!(
                            node.pressure_pa.to_bits(),
                            held.pressure().value().to_bits()
                        );
                        assert!(
                            node.relative_mixing_ratio > 0.0 && node.relative_mixing_ratio <= 1.0,
                            "{name}: {node:?}"
                        );
                    }
                    assert_eq!(
                        (sent.base_pa, sent.top_pa),
                        (
                            profile[0].pressure_pa,
                            profile[profile.len() - 1].pressure_pa
                        )
                    );
                }
                (layer, profile) => panic!("{name}: {layer:?} sent as {profile:?}"),
            }
        }
    }

    /// P14.T54.b.b (decision-p14-t54b-open §2.2): an inventory is sent as the record holds it, each
    /// value to the bit and every list in the record's order: an oxygenated Earth's, a Venus's and
    /// a Titan's by hand. A mode's `deck` is its deck's condensate where the inventory lists a deck
    /// of it, and `null` for a rule's mode (the sulphate layer, the haze), never the material the
    /// sim names as its deck; a member not modelled is absent with the one reason (the haze's
    /// column and temperature, `no_mass_extinction`); a profile's mixing ratios are over its peak,
    /// 1 at the peak; the condensables not formed carry their reasons (the bulk N₂ and CO₂).
    #[test]
    fn an_aerosol_inventory_is_sent_as_the_record_holds_it() {
        let mut counts = ModesSent::default();
        for (name, air) in [
            ("Earth", &EARTH_AIR),
            ("Venus", &VENUS_AIR),
            ("Titan", &TITAN_AIR),
        ] {
            let inventory = air.inventory();
            let dto = aerosol_inventory(&inventory);
            assert_modes_sent(name, &inventory, &dto.modes, &mut counts);
            assert_absorbers_sent(name, inventory.absorbers(), &dto.absorbers);
            assert_eq!(dto.decks.len(), inventory.decks().len(), "{name}");
            for (sent, deck) in dto.decks.iter().zip(inventory.decks()) {
                assert_eq!(
                    (
                        sent.condensate.as_str(),
                        sent.base_pa.to_bits(),
                        sent.base_temperature_k.to_bits(),
                        sent.top_pa.to_bits(),
                        sent.vapour_mole_fraction_below.to_bits(),
                        sent.vapour_mole_fraction_above.to_bits(),
                        sent.f_sed.to_bits(),
                        sent.extrapolated,
                    ),
                    (
                        key(deck.condensate()).as_str(),
                        deck.base().value().to_bits(),
                        deck.base_temperature().value().to_bits(),
                        deck.top().value().to_bits(),
                        deck.vapour_below().to_bits(),
                        deck.vapour_above().to_bits(),
                        deck.sedimentation().to_bits(),
                        deck.extrapolated(),
                    ),
                    "{name}"
                );
            }
            let not_formed: Vec<(&str, &NotFormedReasonDto)> = dto
                .not_formed
                .iter()
                .map(|entry| (entry.condensate.as_str(), &entry.reason))
                .collect();
            let bulk = match name {
                "Venus" => "CO2",
                _ => "N2",
            };
            assert!(
                not_formed.contains(&(bulk, &NotFormedReasonDto::BulkGas)),
                "{name}: {not_formed:?}"
            );
        }
        assert_eq!(
            counts,
            ModesSent {
                layers: 1,
                acid_modes: 4,
                hazes: 1
            },
            "the Earth's sulphate layer, the Venus's acid modes and the Titan's haze"
        );
    }

    /// P14.T54.b.b: each of the sim's reasons a mode's size or optics are not modelled is sent as
    /// one of the closed reason's six (decision-p14-t54b-open §2.2), a grain density's with its
    /// phase and temperature.
    #[test]
    fn the_sim_s_refusals_are_sent_as_the_closed_reason() {
        use hyperion_sim::substance::BuildSizeDistributionError;

        let water = substance::by_key("H2O").expect("a registry row");
        let key_of = water.substance().key();
        let t = Kelvin::new(250.0);
        assert_eq!(
            size_not_modelled(SizeModeError::NoDensity {
                key: key_of,
                phase: CondensedPhase::Liquid,
                temperature: t,
            }),
            AerosolNotModelledDto::NoGrainDensity {
                phase: SubstancePhaseDto::Liquid,
                temperature_k: 250.0,
            }
        );
        assert_eq!(
            size_not_modelled(SizeModeError::NoCollisionConstants { key: key_of }),
            AerosolNotModelledDto::NoCollisionConstants
        );
        assert_eq!(
            size_not_modelled(SizeModeError::NoCondensate),
            AerosolNotModelledDto::NoCondensate
        );
        assert_eq!(
            size_not_modelled(SizeModeError::Distribution(
                BuildSizeDistributionError::Radius { radius_m: -1.0 }
            )),
            AerosolNotModelledDto::NoSizeDistribution
        );
        assert_eq!(
            optics_not_modelled(ResolveAerosolError::NoDensity {
                key: key_of,
                phase: CondensedPhase::Solid,
                temperature: t,
            }),
            AerosolNotModelledDto::NoGrainDensity {
                phase: SubstancePhaseDto::Solid,
                temperature_k: 250.0,
            }
        );
        for refusal in [
            ResolveAerosolError::NotModelled { key: key_of },
            ResolveAerosolError::NoFile {
                key: key_of,
                phase: CondensedPhase::Solid,
            },
            ResolveAerosolError::OffGrid {
                kind: SizeDistributionKind::Gamma,
                effective_radius: Metres::new(1e-10),
                effective_variance: 0.1,
            },
            ResolveAerosolError::NonFiniteTemperature {
                temperature: Kelvin::new(f64::NAN),
            },
        ] {
            assert_eq!(
                optics_not_modelled(refusal),
                AerosolNotModelledDto::OpticsNotModelled,
                "{refusal:?}"
            );
        }
    }

    /// P14.T54.b.b: a rule's mode whose optics are not modelled (solid krypton's, which no index
    /// file serves) is sent with neither its column mass nor its τ(550), never 0
    /// (decision-p14-t54b-open §2.2: `of_rule`'s column is not modelled with its optics), and
    /// with its size and temperature and the optics' reason.
    #[test]
    fn a_rule_s_mode_without_optics_is_sent_without_its_column_or_tau() {
        use hyperion_sim::planetary::chem::condense::ModeProfile;
        use hyperion_sim::substance::SizeDistribution;
        use hyperion_sim::units::Pascals;

        let krypton = substance::by_key("Kr").expect("a registry row");
        let mode = AerosolMode::of_rule(
            krypton,
            CondensedPhase::Solid,
            ShapeClass::Crystal,
            krypton,
            SizeDistribution::gamma(Metres::new(1e-6), 0.1).expect("a gamma distribution"),
            0.5,
            ModeProfile::new(Pascals::new(1e4), Pascals::new(1e3), 0.0).expect("a profile"),
            1.0,
            Kelvin::new(100.0),
        )
        .expect("a rule's mode");
        // Krypton is no aerosol material (a noble gas's optics are its Rayleigh scattering alone).
        assert!(
            matches!(
                mode.optical_depth(),
                Err(ResolveOpticalDepthError::Aerosol(
                    ResolveAerosolError::NotModelled { .. }
                ))
            ),
            "{mode:?}"
        );
        let sent = aerosol_mode(&mode, &[]);
        assert_eq!(
            (
                sent.deck.as_ref(),
                sent.size_distribution.is_some(),
                sent.column_mass_kg_m2,
                sent.tau_550,
                sent.tau_basis,
                sent.temperature_k,
                sent.not_modelled,
            ),
            (
                None,
                true,
                None,
                None,
                None,
                Some(100.0),
                Some(AerosolNotModelledDto::OpticsNotModelled)
            )
        );
        let wire = serde_json::to_value(&sent).expect("a mode serialises");
        for member in ["column_mass_kg_m2", "tau_550", "tau_basis"] {
            assert!(wire.get(member).is_none(), "{member}: {wire}");
        }
    }

    /// P14.T54.b.b: the empty inventory, a surface with no air's (an airless body's, or a melt's
    /// whose volatiles are gone), is sent as the empty object of four empty lists, never `null`.
    #[test]
    fn the_empty_inventory_is_sent_as_four_empty_lists() {
        let empty = AerosolInventory::empty();
        assert_eq!(
            later(RecordModelled::Ok(&empty), |inventory| Some(
                aerosol_inventory(inventory)
            )),
            Modelled::Value(AerosolInventoryDto {
                modes: Vec::new(),
                decks: Vec::new(),
                absorbers: Vec::new(),
                not_formed: Vec::new(),
            })
        );
        assert_eq!(
            serde_json::to_value(aerosol_inventory(&empty)).expect("an inventory serialises"),
            json!({ "modes": [], "decks": [], "absorbers": [], "not_formed": [] })
        );
    }

    /// P14.T54.a: every `ok` surface sends its record's published condensates (P14.T51.a), a
    /// list, never absent or `null`: the record's entries in its order, each substance by its
    /// registry key and phase, its mass, layer, area, trapped area and seasonal flag to the bit,
    /// the layer absent where the record has none and the trapped area while the record holds no
    /// cold traps. An Earth's hold `H2O`: every temperate surface with liquid water whose liquids
    /// list water sends a liquid `H2O` entry with a layer.
    #[test]
    fn every_surface_sends_its_record_s_condensates() {
        let (mut surfaces, mut entries, mut earths, mut unlayered) = (0, 0, 0, 0);
        for (sim, dto, wire) in ok_surfaces() {
            let published = sim.condensates();
            let Modelled::Value(sent) = &dto.condensates else {
                panic!("{}: the condensates are sent", wire.id);
            };
            assert_eq!(sent.len(), published.entries().len(), "{}", wire.id);
            for (dto, entry) in sent.iter().zip(published.entries()) {
                assert_eq!(dto.substance, key(entry.substance()), "{}", wire.id);
                let phase = match entry.phase() {
                    CondensatePhase::Solid => SubstancePhaseDto::Solid,
                    CondensatePhase::Liquid => SubstancePhaseDto::Liquid,
                    CondensatePhase::Supercritical => SubstancePhaseDto::Supercritical,
                };
                assert_eq!(dto.phase, phase, "{}", wire.id);
                assert_eq!(
                    dto.reservoir_kg_m2.to_bits(),
                    entry.reservoir().value().to_bits(),
                    "{}",
                    wire.id
                );
                assert_eq!(
                    dto.reservoir_gel_m.map(f64::to_bits),
                    entry.reservoir_gel().map(|gel| gel.value().to_bits()),
                    "{}",
                    wire.id
                );
                assert_eq!(
                    dto.area_fraction.to_bits(),
                    entry.area_fraction().to_bits(),
                    "{}",
                    wire.id
                );
                assert_eq!(
                    dto.trapped_area_fraction.map(f64::to_bits),
                    Surface::cold_traps_modelled()
                        .then_some(entry.trapped_area_fraction().to_bits()),
                    "{}",
                    wire.id
                );
                assert_eq!(dto.seasonal, entry.is_seasonal(), "{}", wire.id);
                unlayered += usize::from(dto.reservoir_gel_m.is_none());
            }
            let earth = dto.state == SurfaceStateDto::Temperate
                && dto.liquid_water
                && dto
                    .surface_liquids
                    .iter()
                    .any(|area| area.substance == "H2O" && area.area_fraction > 0.0);
            if earth {
                assert!(
                    sent.iter().any(|c| c.substance == "H2O"
                        && c.phase == SubstancePhaseDto::Liquid
                        && c.reservoir_gel_m.is_some_and(|gel| gel > 0.0)),
                    "{}: {sent:?}",
                    wire.id
                );
                earths += 1;
            }
            surfaces += 1;
            entries += sent.len();
        }
        eprintln!(
            "{surfaces} surfaces, {entries} condensates, {unlayered} with no layer, {earths} Earths"
        );
        assert!(
            surfaces > 50 && entries > 50 && earths > 0,
            "{surfaces} {entries} {earths}"
        );
    }

    /// The member of a sent crust that lies at the surface, by its key.
    fn lid_key(crust: &CrustCompositionDto) -> Option<&str> {
        match crust.lid {
            CrustLidDto::Primary => crust.primary.as_deref(),
            CrustLidDto::Secondary => crust.secondary.as_deref(),
            CrustLidDto::Bulk => crust.bulk.as_deref(),
        }
    }

    /// P14.T54.a: every `ok` surface sends its record's crust (P14.T51.c), and every golden solid
    /// surface's record holds one: each crust and the provinces by registry key, the lid, the
    /// redox class, the mantle's ΔIW and the melt area, the record's to the bit. The wire keeps
    /// the record's guarantees: `bulk` alone or at least one of `primary` and `secondary`, no
    /// `tertiary` without a `secondary`, a `lid` naming a present member, the redox class the
    /// ΔIW's ([`RedoxClass::of`]), and the melt area in `[0, 1]`: 1 in the magma-ocean state, and
    /// otherwise 0 on an `H2O` lid, whose liquid is a condensate.
    #[test]
    fn every_solid_surface_sends_its_record_s_crust() {
        let (mut surfaces, mut pools, mut ice_lids) = (0, 0, 0);
        for (sim, dto, wire) in ok_surfaces() {
            let id = wire.id.as_str();
            let RecordModelled::Ok(record) = sim.crust() else {
                panic!("{id}: a golden solid surface's record holds its crust");
            };
            let Modelled::Value(sent) = &dto.crust else {
                panic!("{id}: the crust is sent");
            };
            let keys = [
                record.primary(),
                record.secondary(),
                record.tertiary(),
                record.bulk(),
                Some(record.provinces()),
            ]
            .map(|substance| substance.map(key));
            assert_eq!(
                [
                    sent.primary.clone(),
                    sent.secondary.clone(),
                    sent.tertiary.clone(),
                    sent.bulk.clone(),
                    Some(sent.provinces.clone()),
                ],
                keys,
                "{id}"
            );
            let lid = match record.lid() {
                LidCrust::Primary => CrustLidDto::Primary,
                LidCrust::Secondary => CrustLidDto::Secondary,
                LidCrust::Bulk => CrustLidDto::Bulk,
            };
            let redox = |class: RedoxClass| match class {
                RedoxClass::Reduced => RedoxClassDto::Reduced,
                RedoxClass::Intermediate => RedoxClassDto::Intermediate,
                RedoxClass::Oxidised => RedoxClassDto::Oxidised,
            };
            assert_eq!((sent.lid, sent.redox), (lid, redox(record.redox())), "{id}");
            assert_eq!(
                (
                    sent.mantle_redox_iw.to_bits(),
                    sent.melt_area_fraction.to_bits()
                ),
                (
                    record.mantle_delta_iw().value().to_bits(),
                    record.melt_area_fraction().to_bits()
                ),
                "{id}"
            );
            if sent.bulk.is_some() {
                assert_eq!(
                    (&sent.primary, &sent.secondary, &sent.tertiary),
                    (&None, &None, &None),
                    "{id}: a bulk lies alone"
                );
            } else {
                assert!(
                    sent.primary.is_some() || sent.secondary.is_some(),
                    "{id}: {sent:?}"
                );
            }
            assert!(
                sent.tertiary.is_none() || sent.secondary.is_some(),
                "{id}: {sent:?}"
            );
            let Some(lid_key) = lid_key(sent) else {
                panic!("{id}: the lid names no present member: {sent:?}");
            };
            assert!(sent.mantle_redox_iw.is_finite(), "{id}: {sent:?}");
            assert_eq!(
                sent.redox,
                redox(RedoxClass::of(Dex::new(sent.mantle_redox_iw))),
                "{id}: the class is the ΔIW's"
            );
            let melt = sent.melt_area_fraction;
            assert!((0.0..=1.0).contains(&melt), "{id}: {melt}");
            if dto.state == SurfaceStateDto::MagmaOcean {
                assert_eq!(melt.to_bits(), 1.0_f64.to_bits(), "{id}");
            } else if lid_key == "H2O" {
                assert_eq!(
                    melt.to_bits(),
                    0.0_f64.to_bits(),
                    "{id}: an ice lid melts no rock"
                );
                ice_lids += 1;
            }
            pools += usize::from(melt > 0.0 && melt < 1.0);
            surfaces += 1;
        }
        eprintln!("{surfaces} crusts, {pools} with a melt pool, {ice_lids} H2O lids");
        assert!(
            surfaces > 50 && pools > 0 && ice_lids > 0,
            "{surfaces} {pools} {ice_lids}"
        );
    }

    /// P14.T54.a: a crust's `mantle_redox_iw` is T50.a's inventory's ΔIW, the body's bulk
    /// composition's that the server-only hooks repeat
    /// ([`PlanetarySystem::hooks_at`](hyperion_sim::planetary::PlanetarySystem::hooks_at)), to
    /// the bit, on every solid surface of three golden systems with planets, belt members and
    /// regular moons: the close binary, the subgiant and the third filler.
    #[test]
    fn a_crust_s_mantle_redox_is_its_element_inventory_s() {
        let (mut checked, mut moons) = (0, 0);
        for raw in [
            0x4200_2cb2_0000_0009_u64,
            0x4200_6c9a_0000_0004,
            0x4200_2cb2_0000_0000,
        ] {
            let id = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
            let ctx =
                SystemContext::for_system(&GOLDEN_GALAXY, id).expect("a pinned ID names a system");
            let planets = hyperion_sim::planetary::generate(GOLDEN_GALAXY.seed(), &ctx);
            let snapshot = planets
                .snapshot_at(&ctx, UniverseTime::EPOCH)
                .degrade(DetailLevel::Full);
            for record in snapshot.bodies() {
                let SectionDto::Ok(surface) = body_record(record).surface else {
                    continue;
                };
                let Modelled::Value(sent) = surface.crust else {
                    continue;
                };
                let hooks = planets
                    .hooks_at(&ctx, record.index(), UniverseTime::EPOCH)
                    .expect("a body of the snapshot resolves")
                    .expect("a body with a surface has hooks");
                let Section::Ok(bulk) = hooks.bulk() else {
                    panic!(
                        "{:?}: a body with a crust has a bulk composition",
                        record.index()
                    );
                };
                assert_eq!(
                    sent.mantle_redox_iw.to_bits(),
                    bulk.element_inventory().mantle_delta_iw().value().to_bits(),
                    "{:?}: sent {} against the inventory's {}",
                    record.index(),
                    sent.mantle_redox_iw,
                    bulk.element_inventory().mantle_delta_iw().value()
                );
                checked += 1;
                moons += usize::from(matches!(record.identity().kind(), BodyKind::Moon(_)));
            }
        }
        assert!(checked > 20 && moons > 0, "{checked} crusts, {moons} moons");
    }

    /// P14.T54.a, rust-dev's wire-form rule: a sent crust is an object of its nine members, the
    /// crusts and the provinces registry keys or `null`, `lid` and `redox` their snake-case
    /// names and the two figures numbers, read back as sent. The close binary's belt member
    /// `e201`, a locked airless lava world with a dayside pool, is pinned whole.
    #[test]
    fn a_crust_s_wire_form_is_pinned() {
        let members = [
            "primary",
            "secondary",
            "tertiary",
            "bulk",
            "lid",
            "provinces",
            "redox",
            "mantle_redox_iw",
            "melt_area_fraction",
        ];
        let e201 = BodyIdHex::from_parts(0x4200_2cb2_0000_0009, 0xe201);
        let mut pinned = None;
        for (sim, dto, wire) in ok_surfaces() {
            let id = wire.id.as_str();
            let Modelled::Value(sent) = &dto.crust else {
                panic!("{id}: the crust is sent");
            };
            let surface = serde_json::to_value(dto).expect("a surface section serializes");
            let form = &surface["crust"];
            let fields = form.as_object().expect("a crust is an object");
            let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
            names.sort_unstable();
            let mut expected = members;
            expected.sort_unstable();
            assert_eq!(names, expected, "{id}");
            for member in ["primary", "secondary", "tertiary", "bulk"] {
                assert!(
                    form[member].is_null() || form[member].is_string(),
                    "{id}: {member} {form}"
                );
            }
            assert!(form["provinces"].is_string(), "{id}: {form}");
            assert!(
                ["primary", "secondary", "bulk"].contains(&form["lid"].as_str().unwrap_or("")),
                "{id}: {form}"
            );
            assert!(
                ["reduced", "intermediate", "oxidised"]
                    .contains(&form["redox"].as_str().unwrap_or("")),
                "{id}: {form}"
            );
            for member in ["mantle_redox_iw", "melt_area_fraction"] {
                assert!(form[member].is_f64(), "{id}: {member} {form}");
            }
            let read: CrustCompositionDto =
                serde_json::from_value(form.clone()).expect("a sent crust reads back");
            assert_eq!(&read, sent, "{id}");
            if wire.id == e201 {
                pinned = Some((sim, form.clone()));
            }
        }
        let Some((sim, form)) = pinned else {
            panic!("{e201} has no ok surface: a re-pin of the golden systems re-pins this test");
        };
        let RecordModelled::Ok(record) = sim.crust() else {
            panic!("{e201}: its record holds its crust");
        };
        assert_eq!(
            form,
            json!({
                "primary": null,
                "secondary": "basalt_reduced",
                "tertiary": null,
                "bulk": null,
                "lid": "secondary",
                "provinces": "basalt_reduced",
                "redox": "reduced",
                // The record's values at generator version 22, as literals, the protocol's
                // fixture's (`body_detail_lava_world`), so that the pin does not read back what it
                // pins.
                "mantle_redox_iw": -6.5,
                "melt_area_fraction": 0.249_954_828_560_677_97,
            })
        );
        let melt = record.melt_area_fraction();
        assert!(
            sim.state() == SurfaceState::Airless && melt > 0.0 && melt < 1.0,
            "{e201}: an airless dayside pool, {melt}"
        );
    }

    /// Decision-p14-t54a-water-consistency §5.3: while the record holds no cold traps (until
    /// P14.T54.b's bump at generator version 24), no condensate sends its trapped area, which a
    /// reader takes as not modelled, never as none.
    #[test]
    fn no_condensate_sends_its_trapped_area_while_the_cold_traps_are_held() {
        assert_eq!(
            Surface::cold_traps_modelled(),
            hyperion_sim::GENERATOR_VERSION.get() >= 24,
            "the cold traps are on the record from P14.T54.b's bump"
        );
        let mut entries = 0;
        for (_, dto, wire) in ok_surfaces() {
            let Modelled::Value(sent) = &dto.condensates else {
                panic!("{}: the condensates are sent", wire.id);
            };
            for condensate in sent {
                assert_eq!(
                    condensate.trapped_area_fraction.is_some(),
                    Surface::cold_traps_modelled(),
                    "{}: {condensate:?}",
                    wire.id
                );
                entries += 1;
            }
        }
        assert!(entries > 50, "{entries}");
    }

    /// Decision-p14-t54a-water-consistency §4.2, the wire's own consistency: on every `ok` surface
    /// `surface_liquids` and `surface_ices` are the cover its `condensates` give (W1–W3). Each
    /// list's leading items are that phase's condensates, in their order, at their areas: to the
    /// bit where the list is not scaled, and each divided by one total where it is, to the bit
    /// where nothing is appended. At most one `H2O` item follows them, only where no `H2O`
    /// condensate of the phase is sent. Each list sums to at most 1 (W5), to 10⁻¹², and
    /// `ocean_fraction` and `ice_fraction` are their lists' sums held at 1, to the bit: a list
    /// divided by its total sums back to 1 only to rounding, as `2200165980000000.e201`'s ices
    /// come to 1.000 000 000 000 000 2.
    #[test]
    fn the_wire_s_cover_is_its_condensates_by_their_rule() {
        let (mut surfaces, mut scaled) = (0, 0);
        let (mut crust_ice, mut under_ice) = (0, 0);
        for (_, dto, wire) in ok_surfaces() {
            let Modelled::Value(sent) = &dto.condensates else {
                panic!("{}: the condensates are sent", wire.id);
            };
            for (list, phase, fraction) in [
                (
                    &dto.surface_liquids,
                    SubstancePhaseDto::Liquid,
                    dto.ocean_fraction,
                ),
                (
                    &dto.surface_ices,
                    SubstancePhaseDto::Solid,
                    dto.ice_fraction,
                ),
            ] {
                let entries: Vec<&SurfaceCondensateDto> =
                    sent.iter().filter(|c| c.phase == phase).collect();
                let extra = list
                    .len()
                    .checked_sub(entries.len())
                    .unwrap_or_else(|| panic!("{}: {phase:?} entries unlisted", wire.id));
                assert!(extra <= 1, "{}: {list:?} against {entries:?}", wire.id);
                if let (1, Some(last)) = (extra, list.last()) {
                    assert_eq!(last.substance, "H2O", "{}", wire.id);
                    assert!(
                        entries.iter().all(|c| c.substance != "H2O"),
                        "{}: {list:?} against {entries:?}",
                        wire.id
                    );
                    if last.area_fraction > 0.0 {
                        match phase {
                            SubstancePhaseDto::Liquid => under_ice += 1,
                            SubstancePhaseDto::Solid
                            | SubstancePhaseDto::Supercritical
                            | SubstancePhaseDto::Vapour => crust_ice += 1,
                        }
                    }
                }
                for (item, entry) in list.iter().zip(&entries) {
                    assert_eq!(item.substance, entry.substance, "{}", wire.id);
                }
                let unscaled = list.iter().zip(&entries).all(|(item, entry)| {
                    item.area_fraction.to_bits() == entry.area_fraction.to_bits()
                });
                let listed = list.iter().fold(0.0, |sum, item| sum + item.area_fraction);
                if !unscaled {
                    scaled += 1;
                    assert!((listed - 1.0).abs() <= 1e-12, "{}: {list:?}", wire.id);
                    // Where nothing is appended the total is the entries' own, in their order;
                    // where an item is, every entry shares one ratio to its listed area.
                    let total = entries.iter().fold(0.0, |sum, c| sum + c.area_fraction);
                    let mut ratio = None;
                    for (item, entry) in list.iter().zip(&entries) {
                        if extra == 0 {
                            assert_eq!(
                                item.area_fraction.to_bits(),
                                (entry.area_fraction / total).to_bits(),
                                "{}: {list:?}",
                                wire.id
                            );
                        } else if entry.area_fraction > 0.0 {
                            let this = item.area_fraction / entry.area_fraction;
                            let first = *ratio.get_or_insert(this);
                            assert!(
                                this < 1.0 && (this / first - 1.0).abs() <= 1e-12,
                                "{}: {list:?}",
                                wire.id
                            );
                        }
                    }
                }
                assert!(listed <= 1.0 + 1e-12, "{}: {list:?}", wire.id);
                let held = if listed > 1.0 { 1.0 } else { listed };
                assert_eq!(fraction.to_bits(), held.to_bits(), "{}: {phase:?}", wire.id);
            }
            surfaces += 1;
        }
        eprintln!(
            "{surfaces} surfaces: appended H2O with an area, {crust_ice} ices (W3 a) and \
             {under_ice} oceans under ice (W3 b); {scaled} scaled lists"
        );
        assert!(surfaces > 50, "{surfaces}");
    }

    /// P14.T35.e: the gases sum to 1 to 10⁻⁹ and descend, an airless body's are empty, a body with
    /// no gas has no mixture figures (`null`), and every fraction lies in `[0, 1]`, the liquids and
    /// ices summing to the ocean and ice fractions.
    ///
    /// A body of another state may hold no volatile air either: the close binary's dwarf planet
    /// `e101`, a magma ocean at 1,684 K whose volatiles escape has taken, sends 0 Pa and no gas.
    /// That zero is the volatiles'; the melt's own vapour is not modelled, and `silicate_vapour` is
    /// absent (P14.T35.f).
    #[test]
    fn the_surface_s_mixture_and_fractions_are_in_range() {
        let mut airless = 0;
        for (_, dto, wire) in ok_surfaces() {
            let id = wire.id.as_str();
            let gases = &dto.gases;
            if dto.state == SurfaceStateDto::Airless {
                assert!(gases.is_empty(), "{id}");
                airless += 1;
            }
            if gases.is_empty() {
                assert_eq!(
                    (dto.mean_molar_mass_g_per_mol, dto.heat_capacity_over_r),
                    (None, None),
                    "{id}"
                );
            } else {
                let sum = gases.iter().fold(0.0, |sum, gas| sum + gas.mole_fraction);
                assert!((sum - 1.0).abs() <= 1e-9, "{id}: {sum}");
                assert!(
                    gases
                        .windows(2)
                        .all(|pair| pair[0].mole_fraction >= pair[1].mole_fraction),
                    "{id}: {gases:?}"
                );
                assert!(
                    dto.mean_molar_mass_g_per_mol.is_some_and(|mu| mu > 0.0),
                    "{id}"
                );
            }
            let fractions = [
                dto.ocean_fraction,
                dto.ice_fraction,
                dto.cloud_fraction,
                dto.continental_fraction,
            ];
            assert!(fractions.iter().all(|f| (0.0..=1.0).contains(f)), "{id}");
            for (areas, total) in [
                (&dto.surface_liquids, dto.ocean_fraction),
                (&dto.surface_ices, dto.ice_fraction),
            ] {
                let sum = areas.iter().fold(0.0, |sum, area| sum + area.area_fraction);
                assert!((sum - total).abs() <= 1e-12, "{id}");
            }
        }
        assert!(airless > 0, "no airless body was checked");
    }

    /// P14.T35.f (decision-backlog-2 §2): every surface sends the melt's own vapour absent, with
    /// no key, the close binary's magma oceans among them; and its dwarf planet `e101`, a magma
    /// ocean at 1,684 K, sends its volatiles' zero: 0 Pa, no gas and no mixture figures.
    #[test]
    fn every_surface_sends_the_melt_s_vapour_absent_and_e101_the_volatiles_zero() {
        let close_binary = 0x4200_2cb2_0000_0009;
        let e101 = BodyIdHex::from_parts(close_binary, 0xe101);
        let mut magma_oceans = Vec::new();
        let mut pinned = None;
        for (_, dto, wire) in ok_surfaces() {
            let id = wire.id.as_str();
            assert_eq!(dto.silicate_vapour, Modelled::NotModelled, "{id}");
            let sent = serde_json::to_value(dto).expect("a surface section serializes");
            assert!(sent.get("silicate_vapour").is_none(), "{id}: {sent}");
            if dto.state == SurfaceStateDto::MagmaOcean {
                magma_oceans.push(wire.id.clone());
            }
            if wire.id == e101 {
                pinned = Some(dto);
            }
        }
        for index in [0xe101, 0xe102, 0xe103] {
            let id = BodyIdHex::from_parts(close_binary, index);
            assert!(
                magma_oceans.contains(&id),
                "{id} is not among the magma oceans {magma_oceans:?}"
            );
        }
        let Some(dto) = pinned else {
            panic!("{e101} has no ok surface: a re-pin of the golden systems re-pins this test");
        };
        assert_eq!(
            (dto.state, dto.material),
            (SurfaceStateDto::MagmaOcean, SurfaceMaterialDto::Rock)
        );
        assert_eq!(dto.surface_pressure_pa.to_bits(), 0.0_f64.to_bits());
        assert!(dto.gases.is_empty(), "{:?}", dto.gases);
        assert_eq!(
            (dto.mean_molar_mass_g_per_mol, dto.heat_capacity_over_r),
            (None, None)
        );
        assert_eq!(dto.silicate_vapour, Modelled::NotModelled);
    }

    /// P14.T35.e: every substance the server sends is a registry key, by construction from the
    /// record's `SubstanceId`s.
    #[test]
    fn every_key_the_server_sends_is_a_registry_key() {
        let mut keys = Vec::new();
        for (_, dto, _) in ok_surfaces() {
            keys.extend(dto.gases.iter().map(|gas| gas.species.clone()));
            keys.extend(dto.climate_regime.condensable.clone());
            keys.extend(
                dto.surface_liquids
                    .iter()
                    .map(|area| area.substance.clone()),
            );
            keys.extend(dto.surface_ices.iter().map(|area| area.substance.clone()));
            keys.extend(dto.wet_epoch.as_ref().map(|epoch| epoch.liquid.clone()));
            if let Modelled::Value(ocean) = &dto.subsurface_ocean {
                keys.push(ocean.liquid.clone());
            }
            if let Modelled::Value(condensates) = &dto.condensates {
                keys.extend(condensates.iter().map(|c| c.substance.clone()));
            }
            if let Modelled::Value(crust) = &dto.crust {
                keys.extend(
                    [
                        &crust.primary,
                        &crust.secondary,
                        &crust.tertiary,
                        &crust.bulk,
                    ]
                    .into_iter()
                    .flatten()
                    .cloned(),
                );
                keys.push(crust.provinces.clone());
            }
        }
        assert!(keys.len() > 100, "{} keys", keys.len());
        for key in keys {
            assert!(
                substance::by_key(&key).is_some(),
                "{key} is not a registry key"
            );
        }
    }

    /// P14.T56.a: every golden ring carries its plane's normal on the wire, the sim's
    /// [`Ring::normal`](hyperion_sim::planetary::rings::Ring::normal) to the bit.
    #[test]
    fn every_golden_ring_carries_its_plane_s_normal() {
        let mut rings = 0;
        for (sim, wire) in GOLDEN_RECORDS.iter() {
            let (Section::Ok(Population::Ring(ring)), SectionDto::Ok(PopulationDto::Ring(dto))) =
                (sim.population(), &wire.population)
            else {
                continue;
            };
            let sent = dto.normal.expect("every ring's normal is sent");
            for ((sent_x, sim_x), axis) in sent.into_iter().zip(ring.normal()).zip(["x", "y", "z"])
            {
                assert_eq!(
                    sent_x.to_bits(),
                    sim_x.to_bits(),
                    "{}: the ring's normal's {axis}, sent {sent_x} against the sim's {sim_x}",
                    wire.id
                );
            }
            rings += 1;
        }
        assert!(rings > 0, "no golden system has a ring");
    }

    /// P14.T54.b.d: a solid planet under a supercritical envelope (P14.T53.b part 2, from
    /// generator version 24) is sent its surface `not_applicable` and its envelope `not_modelled`
    /// by their tags alone, as a giant's: neither value's conversion runs, so nothing reaches
    /// [`surface_state`]'s gas-envelope arm or [`envelope`]'s empty match.
    #[test]
    fn a_surface_not_applicable_and_an_envelope_not_modelled_are_sent_by_their_tags() {
        assert_eq!(
            section(&Section::<Surface>::NotApplicable, surface),
            SectionDto::NotApplicable
        );
        assert_eq!(
            section(&Section::<Envelope>::NotModelled, |inner| envelope(*inner)),
            SectionDto::NotModelled
        );
    }
}
