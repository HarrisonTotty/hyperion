//! The pins of rendering plan R12's performance scenes over a real socket (R12.T5.b; the plan's
//! Design notes 8 and 9): the universe seed, systems and bodies of `@hyperion/perf`'s `PERF_PINS`
//! (`packages/perf/src/scenes.ts`), which the scenes of
//! `apps/hyperion/src/renderer/src/view/perf/scenes.ts` fly and the client's perf mode places the
//! ship by, each asked of the server and held to what its scene needs.
//!
//! - `cockpit-descent` and `station-wireframe-descent` descend onto, and `arrival-memory` arrives
//!   at, [`EARTH_SIZED_WORLD`]: a planet of 0.8–1.25 Earth radii whose `body_detail` gives an
//!   air, an ocean fraction above 0.3 with land left to descend onto, clouds and polar ice, and
//!   whose surface the server serves, so that the descent has terrain to fly.
//! - `ringed-giant-approach` flies to [`RINGED_GIANT`], a gas giant of the same system with a
//!   modelled envelope and a massive ring, from 10¹⁰ m to inside the ring's annulus.
//! - `nuclear-disc-sky` sweeps a camera beside [`NUCLEAR_DISC_SYSTEM`]'s barycentre, a system the
//!   galaxy drew from its nuclear disc.
//!
//! A pin is the generator's: a generator version that moves one fails its test here, whose
//! message says to re-pin it in the same commit, as goldens are re-blessed, and to bump the version
//! of every scene that names it in `scenes.ts`, so that records from before and after never compare
//! (Design note 8). Each ID is written in the wire's text form beside its number, since
//! `PERF_PINS` holds the text and a Vitest holds the two files to one another by reading this one.
//!
//! # The search
//!
//! The pins were found at generator version 31 ([`PINNED_AT`]) by
//!
//! ```text
//! HYPERION_PERF_SEARCH=1 \
//!     cargo test -p hyperion-server --test perf_scenes -- --ignored search_pins --nocapture
//! ```
//!
//! which [`search_pins`] runs, printing what it finds and the pins it chooses; without the
//! variable it returns at once, so that `just test-slow`, which runs every ignored test, does not
//! hold every core for five minutes. In a universe of
//! [`SEED`], the other integration tests' own, it asks a server with a worker per core for the
//! systems of the layer-C cells (32 ly, primaries of 0.75–2.5 M☉) of [`SOLAR_CIRCLE_CELLS`], along
//! the solar circle 26,000 ly out on the +y axis (`placeShip.ts`' `DEFAULT_NEAR_LY`), nearest
//! [`SOLAR_CIRCLE_LY`] first: `system_bodies` at the `bulk` level, then `body_detail` for each
//! Earth-sized rocky planet ([`world_misses`]'s criteria) and for each gas giant with a ring, and
//! its rings ([`giant_misses`]'s). It stops at the nearest system holding both. For the nuclear
//! disc it reads the systems the sim places in the 27 layer-C cells about the point one
//! nuclear-disc scale length out on the same axis ([`nuclear_disc_target_ly`], 382.5 ly for
//! [`SEED`]), and takes the nearest that meets [`nuclear_disc_misses`]'s criteria.
//!
//! At generator version 31 it read the bodies of the nearest 37,520 of the cells' 71,349 systems,
//! out to 2,100 ly, in 288 s of its 293 s (pod, 32 vCPU, a test build, 2026-11-17). They hold 43
//! worlds meeting every criterion, the nearest 16.6 ly out, and 872 ringed giants, the nearest
//! 27.9 ly out, but only [`NEAR_SUN_SYSTEM`] holds both; the nearer 420aecb600000006, 1,389 ly
//! out, would but for its world's ocean, 0.955 of the surface.

mod common;

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    BodyDetailRequest, BodyIdHex, BodyKindDto, BodyRecordDto, BodyStateDto, BodySummaryDto,
    ClimateForcingDto, DetailLevelDto, PlanetClassDto, PopulationDto, QueryModeDto, RequestBody,
    ResponseBody, RingDto, RingKindDto, SectionDto, SeedHex, SurfaceStateDto, SystemBodiesDto,
    SystemBodiesRequest, SystemExistenceDto, SystemIdHex, SystemSummaryDto, SystemSummaryRequest,
    UniverseIdHex, UniverseTime,
};
use hyperion_server::surface::RECORD_TIME;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::placement::{CellKey, Existence, SystemRecord, generate_cell, resolve};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::planetary::surface::{CoarseInputs, ReadCoarseInputsError};
use hyperion_sim::planetary::{BodyIndex, SystemContext, generate};
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The universe seed of every scene: the other integration tests' own.
const SEED: u64 = 0x4d2;

/// [`SEED`] as the wire writes it, 16 lowercase hexadecimal digits.
const SEED_HEX: &str = "00000000000004d2";

/// The generator version the pins were found at.
const PINNED_AT: u32 = 31;

/// The system near the Sun that holds [`EARTH_SIZED_WORLD`] and [`RINGED_GIANT`]: one F5.5 V star
/// of the old thin disc at the solar circle, 26,128 ly from the centre and 47 ly above the plane,
/// 2,100 ly along the circle from [`SOLAR_CIRCLE_LY`].
const NEAR_SUN_SYSTEM: u64 = 0x4210_6cb6_0040_0013;

/// [`NEAR_SUN_SYSTEM`] as the wire writes it.
const NEAR_SUN_SYSTEM_HEX: &str = "42106cb600400013";

/// The body index of the Earth-sized world in [`NEAR_SUN_SYSTEM`].
const EARTH_SIZED_WORLD_INDEX: u16 = 0x0200;

/// The Earth-sized world of `cockpit-descent`, `station-wireframe-descent` and `arrival-memory`,
/// as the wire writes its ID: 1.10 R⊕ at 2.02 au, under 124 kPa of nitrogen, its ocean 0.61 of the
/// surface, its polar ice 0.14 and its cloud 0.67.
const EARTH_SIZED_WORLD: &str = "42106cb600400013.0200";

/// The body index of the ringed gas giant in [`NEAR_SUN_SYSTEM`].
const RINGED_GIANT_INDEX: u16 = 0x0400;

/// The ringed gas giant of `ringed-giant-approach`, as the wire writes its ID: 2.2 Jupiter masses
/// at 5.67 au, its massive rock ring 84,587–134,476 km from its centre, of optical depth 0.115.
const RINGED_GIANT: &str = "42106cb600400013.0400";

/// The system of `nuclear-disc-sky`: a single K2 V star of the galaxy's nuclear disc, 382.8 ly
/// from the centre on the +y axis and 0.5 ly above the plane.
const NUCLEAR_DISC_SYSTEM: u64 = 0x4200_202e_0000_30eb;

/// [`NUCLEAR_DISC_SYSTEM`] as the wire writes it.
const NUCLEAR_DISC_SYSTEM_HEX: &str = "4200202e000030eb";

/// The scenes that name [`EARTH_SIZED_WORLD`] (and [`NEAR_SUN_SYSTEM`]).
const WORLD_SCENES: &str = "`cockpit-descent`, `station-wireframe-descent` and `arrival-memory`";

/// The scene that names [`RINGED_GIANT`].
const GIANT_SCENES: &str = "`ringed-giant-approach`";

/// The scene that names [`NUCLEAR_DISC_SYSTEM`].
const NUCLEAR_DISC_SCENES: &str = "`nuclear-disc-sky`";

/// The search that finds the pins, as the module documentation gives it.
const SEARCH: &str = concat!(
    "HYPERION_PERF_SEARCH=1 ",
    "cargo test -p hyperion-server --test perf_scenes -- --ignored search_pins --nocapture",
);

/// The variable that lets [`search_pins`] search; without it the test returns at once.
const SEARCH_ENV: &str = "HYPERION_PERF_SEARCH";

/// Earth's mean radius, m: the IUGG's mean radius R₁ of the Geodetic Reference System 1980
/// (Moritz 2000, Journal of Geodesy 74, 128), 6,371.0 km.
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// An Earth-sized radius, in Earth radii: Kepler's Earth-size class, 0.8–1.25 R⊕ (Borucki et al.
/// 2011, Astrophys. J. 736, 19).
const EARTH_SIZED_RADII: RangeInclusive<f64> = 0.8..=1.25;

/// The surface pressures of an air the descents fly down through, Pa: from a tenth of Earth's
/// 101.325 kPa, an air whose scattering lights the sky, to ten times it, below which the ground
/// is still seen from orbit through the clouds' gaps.
const SURFACE_PRESSURE_PA: RangeInclusive<f64> = 10_000.0..=1_000_000.0;

/// The ocean fraction the world's ocean must exceed: the plan's (R12.T5.b).
const OCEAN_FRACTION_ABOVE: f64 = 0.3;

/// The most ocean fraction the world's ocean may have: a fifth of the surface is left land, about
/// Earth's 0.29 (its ocean 361 of 510 million km², Eakins and Sharman 2010, NOAA National
/// Geophysical Data Center), so that land is in view through the descents and the site they draw
/// from the survey's cells may lie on it.
const OCEAN_FRACTION_AT_MOST: f64 = 0.8;

/// The ice fraction a world with polar ice stays below: half the surface, beyond which plan 14
/// calls a world iced over (`SurfaceStateDto::Desert`'s "neither iced over half its surface").
const ICE_FRACTION_BELOW: f64 = 0.5;

/// The most a massive ring's outer edge may lie from its giant's centre, m: the approach starts at
/// 10¹⁰ m and crosses the 10⁹ m boundary (Design note 8) before it reaches the ring.
const RING_OUTER_EDGE_BELOW_M: f64 = 1e9;

/// The galactocentric radius within which a system lies in the nuclear disc, ly: 230 pc, the
/// nuclear stellar disc's radius (Launhardt, Zylka and Mezger 2002, A&A 384, 112), at 3.261 564 ly
/// a parsec (IAU 2012 Resolution B2's au and the parsec's definition, IAU 2015 Resolution B2).
const NUCLEAR_DISC_RADIUS_LY: f64 = 230.0 * 3.261_564;

/// The workers of the search's server, and its clients: the machine's parallelism. What the
/// search prints does not depend on it.
fn search_workers() -> NonZeroUsize {
    std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN)
}

/// The layer-C cells the search reads about the solar circle, as ranges of cell coordinates on
/// x, y and z: 256 cells along x, 4,096 ly either side of the +y axis, three along y about
/// 26,000 ly (cell 812 holds 25,984–26,016 ly) and four about the plane, 3,072 cells; their
/// galactocentric radii are 25,952–26,357 ly.
const SOLAR_CIRCLE_CELLS: [RangeInclusive<i32>; 3] = [-128..=127, 811..=813, -2..=1];

/// Where the search measures nearness about the solar circle, ly in the galactic frame:
/// `placeShip.ts`' `DEFAULT_NEAR_LY`, 26,000 ly out on the +y axis.
const SOLAR_CIRCLE_LY: [f64; 3] = [0.0, 26_000.0, 0.0];

/// The instant every pin is asked at: the epoch, the surface service's record time
/// ([`RECORD_TIME`]), at which a descent's coarse field is read.
fn epoch() -> UniverseTime {
    UniverseTime {
        seconds: RECORD_TIME.seconds(),
        nanos: 0,
    }
}

/// The message of a pin's failure: re-pin `pin` and bump the version of `scenes`.
fn repin(pin: &str, scenes: &str) -> String {
    format!(
        "pin moved at generator version {GENERATOR_VERSION} (pinned at {PINNED_AT}): re-pin \
         `{pin}` here and in packages/perf/src/scenes.ts's PERF_PINS (search: {SEARCH}) and bump \
         the version of {scenes} in apps/hyperion/src/renderer/src/view/perf/scenes.ts"
    )
}

/// The galaxy of [`SEED`], built once per test binary.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

/// A server, a client that has said hello, and a universe of [`SEED`] named `name`.
async fn started(name: &str) -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe(name, SEED).await.id;
    (server, client, universe)
}

/// The bodies of `system` at the epoch, at the `bulk` level, which gives each body's class,
/// radius and rings.
async fn bodies(client: &mut TestClient, universe: &UniverseIdHex, system: u64) -> SystemBodiesDto {
    let body = RequestBody::SystemBodies(SystemBodiesRequest {
        universe: universe.clone(),
        system: SystemIdHex::from_u64(system),
        time: epoch(),
        detail: DetailLevelDto::Bulk,
    });
    match client.request(body).await {
        Ok(ResponseBody::SystemBodies(answer)) => *answer,
        other => panic!("expected the bodies of {system:016x}, got {other:?}"),
    }
}

/// The whole record of `body` at the epoch, at the `full` level.
async fn detail(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    body: &BodyIdHex,
) -> BodyRecordDto {
    let request = RequestBody::BodyDetail(BodyDetailRequest {
        universe: universe.clone(),
        body: body.clone(),
        time: epoch(),
        detail: DetailLevelDto::Full,
    });
    match client.request(request).await {
        Ok(ResponseBody::BodyDetail(answer)) => {
            assert_eq!(answer.granted, DetailLevelDto::Full, "{body}");
            answer.record
        }
        other => panic!("expected the record of {body}, got {other:?}"),
    }
}

/// The summary of `system`'s stars at the epoch.
async fn summary(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    system: u64,
) -> SystemSummaryDto {
    let body = RequestBody::SystemSummary(SystemSummaryRequest {
        universe: universe.clone(),
        system: SystemIdHex::from_u64(system),
        time: epoch(),
        mode: QueryModeDto::Now,
    });
    match client.request(body).await {
        Ok(ResponseBody::SystemSummary(answer)) => answer,
        other => panic!("expected the summary of {system:016x}, got {other:?}"),
    }
}

/// The body ID a wire text names.
fn body_id(text: &str) -> BodyIdHex {
    BodyIdHex::try_from(text.to_owned()).expect("a pin is a body ID")
}

/// Whether a list entry may be an Earth-sized world, by what `system_bodies` gives: a present
/// rocky planet of [`EARTH_SIZED_RADII`], whose record [`world_misses`] then reads.
fn may_be_earth_sized(body: &BodySummaryDto) -> bool {
    let SectionDto::Ok(bulk) = &body.bulk else {
        return false;
    };
    body.kind == BodyKindDto::Planet
        && body.state == BodyStateDto::Present
        && bulk.class == PlanetClassDto::Rocky
        && EARTH_SIZED_RADII.contains(&(bulk.radius_m / EARTH_RADIUS_M))
}

/// Whether a list entry may be a ringed gas giant, by what `system_bodies` gives: a present gas
/// giant with a ring, whose records [`giant_misses`] then reads.
fn may_be_ringed_giant(body: &BodySummaryDto) -> bool {
    let (SectionDto::Ok(bulk), SectionDto::Ok(rings)) = (&body.bulk, &body.rings) else {
        return false;
    };
    body.kind == BodyKindDto::Planet
        && body.state == BodyStateDto::Present
        && bulk.class == PlanetClassDto::GasGiant
        && !rings.is_empty()
}

/// What an Earth-sized world's record lacks of what the descents and the arrival need, each with
/// its value; empty when it has everything.
///
/// It is a present rocky planet of [`EARTH_SIZED_RADII`] with a figure and a solid surface, which
/// together are what the server surveys and serves a coarse field from (`placeShip.ts`'
/// `canBeSurveyed`); its surface is `temperate`, under an air of [`SURFACE_PRESSURE_PA`], with an
/// ocean fraction above [`OCEAN_FRACTION_ABOVE`] and at most [`OCEAN_FRACTION_AT_MOST`], so that
/// land is left to descend onto, and a cloud fraction above 0 and below 1. Its ice is polar: water
/// ice under [`ICE_FRACTION_BELOW`] of the surface on a fast rotator's seasonal climate whose poles
/// are colder than its equator, since R09's ice step lays a perennial ice on the field's coldest
/// cells by their annual mean.
fn world_misses(record: &BodyRecordDto) -> Vec<String> {
    let mut misses = Vec::new();
    let mut miss = |what: String| misses.push(what);
    if record.kind != BodyKindDto::Planet {
        miss(format!("kind {:?}, not a planet", record.kind));
    }
    if record.state != BodyStateDto::Present {
        miss(format!("state {:?}, not present", record.state));
    }
    match &record.bulk {
        SectionDto::Ok(bulk) => {
            if bulk.class != PlanetClassDto::Rocky {
                miss(format!("class {:?}, not rocky", bulk.class));
            }
            let radii = bulk.radius_m / EARTH_RADIUS_M;
            if !EARTH_SIZED_RADII.contains(&radii) {
                miss(format!(
                    "radius {radii:.3} R⊕, outside {EARTH_SIZED_RADII:?}"
                ));
            }
        }
        other => miss(format!("bulk {other:?}")),
    }
    if !matches!(record.figure, Some(SectionDto::Ok(_))) {
        miss(format!("figure {:?}, not ok", record.figure));
    }
    let SectionDto::Ok(surface) = &record.surface else {
        miss(format!("surface {:?}, not ok", record.surface));
        return misses;
    };
    if surface.state != SurfaceStateDto::Temperate {
        miss(format!("surface state {:?}, not temperate", surface.state));
    }
    if !SURFACE_PRESSURE_PA.contains(&surface.surface_pressure_pa) {
        miss(format!(
            "surface pressure {} Pa, outside {SURFACE_PRESSURE_PA:?}",
            surface.surface_pressure_pa
        ));
    }
    if !(surface.ocean_fraction > OCEAN_FRACTION_ABOVE
        && surface.ocean_fraction <= OCEAN_FRACTION_AT_MOST)
    {
        miss(format!(
            "ocean fraction {}, not above {OCEAN_FRACTION_ABOVE} and at most \
             {OCEAN_FRACTION_AT_MOST}",
            surface.ocean_fraction
        ));
    }
    if !(surface.cloud_fraction > 0.0 && surface.cloud_fraction < 1.0) {
        miss(format!(
            "cloud fraction {}, not above 0 and below 1",
            surface.cloud_fraction
        ));
    }
    if !(surface.ice_fraction > 0.0 && surface.ice_fraction < ICE_FRACTION_BELOW) {
        miss(format!(
            "ice fraction {}, not above 0 and below {ICE_FRACTION_BELOW}",
            surface.ice_fraction
        ));
    }
    let water_ice = surface
        .surface_ices
        .iter()
        .any(|ice| ice.substance == "H2O" && ice.area_fraction > 0.0);
    if !water_ice {
        miss(format!("no water ice in {:?}", surface.surface_ices));
    }
    if surface.climate_regime.forcing != ClimateForcingDto::Seasonal {
        miss(format!(
            "climate forcing {:?}, not seasonal",
            surface.climate_regime.forcing
        ));
    }
    if surface.equator_pole_contrast_k <= 0.0 {
        miss(format!(
            "equator-pole contrast {} K: the poles are not the colder",
            surface.equator_pole_contrast_k
        ));
    }
    misses
}

/// The first massive ring of `rings`, the records of a giant's rings, with its ID.
fn massive_ring(rings: &[BodyRecordDto]) -> Option<(&BodyIdHex, &RingDto)> {
    rings.iter().find_map(|record| match &record.population {
        SectionDto::Ok(PopulationDto::Ring(ring)) if ring.ring_kind == RingKindDto::Massive => {
            Some((&record.id, ring))
        }
        _ => None,
    })
}

/// What a gas giant's record, with its rings' records, lacks of what the approach needs, each with
/// its value; empty when it has everything.
///
/// It is a present gas giant whose envelope, the atmosphere the approach lights, is modelled, with
/// a massive ring (every giant has a dusty one, of optical depth under 10⁻³) whose outer edge lies
/// inside [`RING_OUTER_EDGE_BELOW_M`].
fn giant_misses(record: &BodyRecordDto, rings: &[BodyRecordDto]) -> Vec<String> {
    let mut misses = Vec::new();
    if record.kind != BodyKindDto::Planet {
        misses.push(format!("kind {:?}, not a planet", record.kind));
    }
    if record.state != BodyStateDto::Present {
        misses.push(format!("state {:?}, not present", record.state));
    }
    match &record.bulk {
        SectionDto::Ok(bulk) if bulk.class == PlanetClassDto::GasGiant => {}
        other => misses.push(format!("bulk {other:?}, not a gas giant's")),
    }
    if !matches!(record.envelope, Some(SectionDto::Ok(_))) {
        misses.push(format!("envelope {:?}, not ok", record.envelope));
    }
    match massive_ring(rings) {
        Some((_, ring)) if ring.outer_edge_m < RING_OUTER_EDGE_BELOW_M => {}
        Some((id, ring)) => misses.push(format!(
            "ring {id}'s outer edge {} m, not inside {RING_OUTER_EDGE_BELOW_M} m",
            ring.outer_edge_m
        )),
        None => misses.push(format!("no massive ring among {:?}", record.rings)),
    }
    misses
}

/// The records of a giant's rings, by its record's `rings`.
async fn ring_records(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    record: &BodyRecordDto,
) -> Vec<BodyRecordDto> {
    let SectionDto::Ok(ids) = &record.rings else {
        return Vec::new();
    };
    let mut rings = Vec::with_capacity(ids.len());
    for id in ids {
        rings.push(detail(client, universe, id).await);
    }
    rings
}

/// The coarse-pass inputs the surface service reads for `body` from its record at the record
/// time, as the production source, `RecordInputs`, reads them: an error is a field the server
/// answers `not_modelled`, or a body with no solid surface.
fn coarse_inputs(body: &BodyIdHex) -> Result<CoarseInputs, ReadCoarseInputsError> {
    let (system, index) = body.to_parts();
    let id = SystemId::from_raw(system).expect("a pin's system is a system ID");
    let ctx = SystemContext::for_system(galaxy(), id).expect("a pin's system resolves");
    let planets = generate(Seed::new(SEED), &ctx);
    let index = BodyIndex::try_from(index).expect("a pin's index is a body index");
    let record = planets
        .body_at(&ctx, index, RECORD_TIME)
        .expect("a pin's body has a record");
    let planet = CoarseInputs::planet_of(&record)
        .map(|planet| planets.body_at(&ctx, planet, RECORD_TIME))
        .transpose()
        .expect("a moon's planet has a record");
    CoarseInputs::for_body(&record, planet.as_ref(), &ctx)
}

/// A system's galactocentric radius in the plane and its height above it, ly, at the epoch.
fn radius_and_height_ly(record: &SystemRecord) -> (f64, f64) {
    let [x, y, z] = record.epoch_position().to_light_years_f64();
    ((x * x + y * y).sqrt(), z)
}

/// A system's distance from `point`, ly in the galactic frame, at the epoch.
fn distance_ly(record: &SystemRecord, point: [f64; 3]) -> f64 {
    let position = record.epoch_position().to_light_years_f64();
    let [x, y, z] = [0, 1, 2].map(|axis| position[axis] - point[axis]);
    (x * x + y * y + z * z).sqrt()
}

/// What a system, by its record and its summary at the epoch, lacks of what the nuclear disc's
/// sweep needs, each with its value; empty when it has everything.
///
/// The galaxy drew it from its nuclear disc, it lies inside [`NUCLEAR_DISC_RADIUS_LY`] of the
/// centre, and it exists at the epoch with a single star, so that the camera's place beside its
/// barycentre (`placeShip.ts`' `barycentre`) looks at a star and not at the empty space between a
/// multiple's.
fn nuclear_disc_misses(record: &SystemRecord, summary: &SystemSummaryDto) -> Vec<String> {
    let mut misses = Vec::new();
    if record.population() != Population::NuclearDisc {
        misses.push(format!(
            "population {}, not the nuclear disc",
            record.population().name()
        ));
    }
    let (radius, height) = radius_and_height_ly(record);
    if radius >= NUCLEAR_DISC_RADIUS_LY {
        misses.push(format!(
            "{radius} ly from the centre ({height} ly from the plane), outside the nuclear disc's \
             {NUCLEAR_DISC_RADIUS_LY} ly"
        ));
    }
    if summary.existence != SystemExistenceDto::Exists {
        misses.push(format!("existence {:?}", summary.existence));
    }
    if summary.stars.len() != 1 {
        misses.push(format!("{} stars, not one", summary.stars.len()));
    }
    misses
}

/// The point the nuclear disc's search measures nearness from, ly in the galactic frame: one of
/// the galaxy's nuclear-disc scale lengths out on the +y axis, the Sun's side, in the plane.
fn nuclear_disc_target_ly() -> [f64; 3] {
    [0.0, galaxy().params().nuclear_disc().length().value(), 0.0]
}

/// The pins' text forms are their IDs': the seed's 16 digits and each body's system and index.
#[test]
fn the_pins_text_forms_are_their_ids() {
    assert_eq!(SeedHex::from_u64(SEED).as_str(), SEED_HEX);
    assert_eq!(
        SystemIdHex::from_u64(NEAR_SUN_SYSTEM).as_str(),
        NEAR_SUN_SYSTEM_HEX
    );
    assert_eq!(
        SystemIdHex::from_u64(NUCLEAR_DISC_SYSTEM).as_str(),
        NUCLEAR_DISC_SYSTEM_HEX
    );
    assert_eq!(
        body_id(EARTH_SIZED_WORLD).to_parts(),
        (NEAR_SUN_SYSTEM, EARTH_SIZED_WORLD_INDEX)
    );
    assert_eq!(
        body_id(RINGED_GIANT).to_parts(),
        (NEAR_SUN_SYSTEM, RINGED_GIANT_INDEX)
    );
}

/// `cockpit-descent`'s, `station-wireframe-descent`'s and `arrival-memory`'s world: `body_detail`
/// gives an Earth-sized planet with an air, an ocean, clouds and polar ice ([`world_misses`]), and
/// the surface service reads its record's coarse inputs, so that it serves the world's field.
#[tokio::test]
async fn the_earth_sized_world_has_an_air_an_ocean_clouds_and_polar_ice() {
    let (server, mut client, universe) = started("Perf scenes world").await;
    let world = body_id(EARTH_SIZED_WORLD);
    let record = detail(&mut client, &universe, &world).await;
    let misses = world_misses(&record);
    assert!(
        misses.is_empty(),
        "{world}: {}; {}",
        misses.join("; "),
        repin("EARTH_SIZED_WORLD", WORLD_SCENES)
    );
    if let Err(error) = coarse_inputs(&world) {
        panic!(
            "{world}: the server reads no coarse inputs from its record: {error:?}; {}",
            repin("EARTH_SIZED_WORLD", WORLD_SCENES)
        );
    }
    client.close().await;
    server.stop().await;
}

/// `ringed-giant-approach`'s giant: `body_detail` gives a gas giant of [`NEAR_SUN_SYSTEM`] with a
/// modelled envelope and a massive ring inside 10⁹ m ([`giant_misses`]).
#[tokio::test]
async fn the_ringed_giant_has_an_envelope_and_a_massive_ring() {
    let (server, mut client, universe) = started("Perf scenes giant").await;
    let giant = body_id(RINGED_GIANT);
    let record = detail(&mut client, &universe, &giant).await;
    let rings = ring_records(&mut client, &universe, &record).await;
    let misses = giant_misses(&record, &rings);
    assert!(
        misses.is_empty(),
        "{giant}: {}; {}",
        misses.join("; "),
        repin("RINGED_GIANT", GIANT_SCENES)
    );
    client.close().await;
    server.stop().await;
}

/// `nuclear-disc-sky`'s system: the galaxy drew it from its nuclear disc, it lies inside
/// [`NUCLEAR_DISC_RADIUS_LY`] of the centre, and `system_summary` gives it existing at the epoch
/// with the single star the camera sits beside ([`nuclear_disc_misses`]).
#[tokio::test]
async fn the_nuclear_disc_system_exists_in_the_nuclear_disc() {
    let (server, mut client, universe) = started("Perf scenes nuclear disc").await;
    let answer = summary(&mut client, &universe, NUCLEAR_DISC_SYSTEM).await;
    let id = SystemId::from_raw(NUCLEAR_DISC_SYSTEM).expect("a system ID");
    let record = resolve(galaxy(), id).expect("the system resolves, as the server answered it");
    let misses = nuclear_disc_misses(&record, &answer);
    assert!(
        misses.is_empty(),
        "{NUCLEAR_DISC_SYSTEM_HEX}: {}; {}",
        misses.join("; "),
        repin("NUCLEAR_DISC_SYSTEM", NUCLEAR_DISC_SCENES)
    );
    client.close().await;
    server.stop().await;
}

/// What the search found in one system: its place in the search's order, its distance from
/// [`SOLAR_CIRCLE_LY`], ly, and each world and giant it asked about, with what each lacks.
struct Found {
    order: usize,
    distance_ly: f64,
    system: u64,
    worlds: Vec<(BodyRecordDto, Vec<String>)>,
    giants: Vec<(BodyRecordDto, Vec<BodyRecordDto>, Vec<String>)>,
}

impl Found {
    /// The system's first world meeting every criterion whose coarse inputs the server reads, and
    /// its first giant meeting every criterion, with its rings, where it holds both.
    fn both(&self) -> Option<(&BodyRecordDto, &BodyRecordDto, &[BodyRecordDto])> {
        let (world, _) = self
            .worlds
            .iter()
            .find(|(record, misses)| misses.is_empty() && coarse_inputs(&record.id).is_ok())?;
        let (giant, rings, _) = self
            .giants
            .iter()
            .find(|(_, _, misses)| misses.is_empty())?;
        Some((world, giant, rings))
    }
}

/// One line of a world's figures as `body_detail` gives them.
fn world_line(record: &BodyRecordDto) -> String {
    let radius = match &record.bulk {
        SectionDto::Ok(bulk) => bulk.radius_m / EARTH_RADIUS_M,
        _ => f64::NAN,
    };
    let mass = match record.mass_kg {
        SectionDto::Ok(mass) => mass,
        _ => f64::NAN,
    };
    let SectionDto::Ok(surface) = &record.surface else {
        return format!(
            "{} radius {radius:.3} R⊕, surface {:?}",
            record.id, record.surface
        );
    };
    format!(
        "{} radius {radius:.4} R⊕ mass {mass:.4e} kg, {:?}, pressure {:.0} Pa, ocean {:.4}, \
         cloud {:.4}, ice {:.4} {:?}, T {:.1} K, equator-pole {:.1} K, {:?}",
        record.id,
        surface.state,
        surface.surface_pressure_pa,
        surface.ocean_fraction,
        surface.cloud_fraction,
        surface.ice_fraction,
        surface
            .surface_ices
            .iter()
            .map(|ice| format!("{} {:.4}", ice.substance, ice.area_fraction))
            .collect::<Vec<_>>(),
        surface.mean_surface_temperature_k,
        surface.equator_pole_contrast_k,
        surface.climate_regime.forcing,
    )
}

/// One line of a giant's figures and its massive ring's as `body_detail` gives them.
fn giant_line(record: &BodyRecordDto, rings: &[BodyRecordDto]) -> String {
    let (radius, class) = match &record.bulk {
        SectionDto::Ok(bulk) => (bulk.radius_m, Some(bulk.class)),
        _ => (f64::NAN, None),
    };
    let mass = match record.mass_kg {
        SectionDto::Ok(mass) => mass,
        _ => f64::NAN,
    };
    let ring = massive_ring(rings).map_or_else(
        || "no massive ring".to_owned(),
        |(id, ring)| {
            format!(
                "ring {id} {:?} {:?} {:.4e}–{:.4e} m, optical depth {:.4}, {} gaps",
                ring.ring_kind,
                ring.material,
                ring.inner_edge_m,
                ring.outer_edge_m,
                ring.optical_depth,
                ring.gaps.len()
            )
        },
    );
    let envelope = matches!(record.envelope, Some(SectionDto::Ok(_)));
    format!(
        "{} {class:?} radius {radius:.4e} m mass {mass:.4e} kg, envelope ok {envelope}, {ring}",
        record.id
    )
}

/// The astronomical unit, m (IAU 2012 Resolution B2).
const AU_M: f64 = 149_597_870_700.0;

/// A pinned body's figures as `body_detail` gives them, one per line, for the pins' record: its
/// mass, orbit, bulk and figure, and its surface, its envelope's state and its rings' extents
/// where it has them.
fn pin_figures(record: &BodyRecordDto, rings: &[BodyRecordDto]) -> Vec<String> {
    let mut lines = vec![format!(
        "{} {:?}, {:?}",
        record.id, record.kind, record.state
    )];
    if let SectionDto::Ok(mass) = record.mass_kg {
        lines.push(format!("mass {mass:.6e} kg"));
    }
    if let SectionDto::Ok(orbit) = &record.orbit {
        lines.push(format!(
            "orbit about {:?}: a {:.6e} m ({:.4} au), e {:.4}, period {:.6e} s",
            orbit.parent,
            orbit.orbit.semi_major_axis_m,
            orbit.orbit.semi_major_axis_m / AU_M,
            orbit.orbit.eccentricity,
            orbit.orbit.period_s
        ));
    }
    if let SectionDto::Ok(bulk) = &record.bulk {
        lines.push(format!(
            "bulk {:?}: radius {} m ({:.4} R⊕), density {:.1} kg m⁻³, gravity {:.3} m s⁻², \
             equilibrium {:.1} K",
            bulk.class,
            bulk.radius_m,
            bulk.radius_m / EARTH_RADIUS_M,
            bulk.density_kg_m3,
            bulk.surface_gravity_m_s2,
            bulk.equilibrium_temperature_k
        ));
    }
    if let Some(SectionDto::Ok(figure)) = &record.figure {
        lines.push(format!(
            "figure: equatorial {} m, polar {} m, {:?}",
            figure.equatorial_radius_m, figure.polar_radius_m, figure.datum
        ));
    }
    if let SectionDto::Ok(surface) = &record.surface {
        let gases = surface
            .gases
            .iter()
            .take(4)
            .map(|gas| format!("{} {:.4}", gas.species, gas.mole_fraction))
            .collect::<Vec<_>>();
        lines.push(format!(
            "surface {:?}: pressure {} Pa, mean {:.2} K, equator-pole {:.2} K, {:?}, \
             gases {gases:?}",
            surface.state,
            surface.surface_pressure_pa,
            surface.mean_surface_temperature_k,
            surface.equator_pole_contrast_k,
            surface.climate_regime.forcing
        ));
        lines.push(format!(
            "fractions: ocean {}, ice {}, cloud {}, continental {}; liquids {:?}; ices {:?}",
            surface.ocean_fraction,
            surface.ice_fraction,
            surface.cloud_fraction,
            surface.continental_fraction,
            surface
                .surface_liquids
                .iter()
                .map(|area| format!("{} {:.4}", area.substance, area.area_fraction))
                .collect::<Vec<_>>(),
            surface
                .surface_ices
                .iter()
                .map(|area| format!("{} {:.4}", area.substance, area.area_fraction))
                .collect::<Vec<_>>()
        ));
    }
    if let Some(SectionDto::Ok(_)) = &record.envelope {
        lines.push("envelope ok".to_owned());
    }
    for ring in rings {
        if let SectionDto::Ok(PopulationDto::Ring(population)) = &ring.population {
            lines.push(format!(
                "ring {} {:?} {:?}: {} – {} m, optical depth {}, {} gaps",
                ring.id,
                population.ring_kind,
                population.material,
                population.inner_edge_m,
                population.outer_edge_m,
                population.optical_depth,
                population.gaps.len()
            ));
        }
    }
    lines
}

/// The search for the pins, by the module documentation's command.
///
/// It reads the systems of [`SOLAR_CIRCLE_CELLS`] nearest [`SOLAR_CIRCLE_LY`] first, and stops
/// once every system nearer than the nearest holding both an Earth-sized world and a ringed giant
/// is read, so that what it prints does not depend on how its clients' requests interleave: every
/// world and giant of those systems that meets its criteria, the worlds that miss one, the system
/// holding both, and the nuclear disc's systems nearest [`nuclear_disc_target_ly`] that meet
/// [`nuclear_disc_misses`]'s criteria; then the pins it chooses, the nearest of each.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "the pins' search, five minutes on 32 cores: by hand with HYPERION_PERF_SEARCH=1"]
#[expect(clippy::too_many_lines, reason = "one search, printed as it goes")]
async fn search_pins() {
    if std::env::var_os(SEARCH_ENV).is_none() {
        eprintln!("search_pins: set {SEARCH_ENV}=1 to search ({SEARCH})");
        return;
    }
    let start = Instant::now();
    let mut systems = Vec::new();
    let mut cell = Vec::new();
    let [xs, ys, zs] = SOLAR_CIRCLE_CELLS;
    for x in xs {
        for y in ys.clone() {
            for z in zs.clone() {
                cell.clear();
                let key = CellKey::new(Layer::C, [x, y, z]).expect("a cell of the grid");
                generate_cell(galaxy(), key, &mut cell);
                for record in &cell {
                    let distance = distance_ly(record, SOLAR_CIRCLE_LY);
                    systems.push((distance, record.id().raw()));
                }
            }
        }
    }
    systems.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let count = systems.len();
    println!(
        "seed {SEED_HEX}, generator version {GENERATOR_VERSION}: {count} systems of layer C in \
         cells {SOLAR_CIRCLE_CELLS:?}, nearest {SOLAR_CIRCLE_LY:?} ly first ({:.1} s)",
        start.elapsed().as_secs_f64()
    );

    let workers = search_workers();
    let temp = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(temp.path()).workers(workers).build();
    let server = TestServer::start_with(config).await;
    let mut first = server.connected().await;
    let universe = first.create_universe("Perf scenes search", SEED).await.id;
    let queue = Arc::new(Mutex::new(
        systems.into_iter().enumerate().collect::<VecDeque<_>>(),
    ));
    // The place in the order of the nearest system holding both found so far: no client takes a
    // system after it, and every one before it is taken.
    let nearest_both = Arc::new(AtomicUsize::new(usize::MAX));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..workers.get() {
        let mut client = server.connected().await;
        let universe = universe.clone();
        let queue = Arc::clone(&queue);
        let nearest_both = Arc::clone(&nearest_both);
        tasks.spawn(async move {
            let mut found = Vec::new();
            loop {
                let next = queue.lock().expect("no task panicked").pop_front();
                let Some((order, (distance_ly, system))) = next else {
                    break;
                };
                if order > nearest_both.load(Ordering::Acquire) {
                    break;
                }
                let answer = bodies(&mut client, &universe, system).await;
                let mut worlds = Vec::new();
                let mut giants = Vec::new();
                for body in &answer.bodies {
                    if may_be_earth_sized(body) {
                        let record = detail(&mut client, &universe, &body.id).await;
                        let misses = world_misses(&record);
                        worlds.push((record, misses));
                    } else if may_be_ringed_giant(body) {
                        let record = detail(&mut client, &universe, &body.id).await;
                        let rings = ring_records(&mut client, &universe, &record).await;
                        let misses = giant_misses(&record, &rings);
                        giants.push((record, rings, misses));
                    }
                }
                let found_here = Found {
                    order,
                    distance_ly,
                    system,
                    worlds,
                    giants,
                };
                if found_here.both().is_some() {
                    nearest_both.fetch_min(order, Ordering::AcqRel);
                }
                found.push(found_here);
            }
            client.close().await;
            found
        });
    }
    let mut found = Vec::with_capacity(count);
    while let Some(done) = tasks.join_next().await {
        found.extend(done.expect("a search task finishes"));
    }
    let last = nearest_both.load(Ordering::Acquire);
    found.retain(|each| each.order <= last);
    found.sort_by_key(|each| each.order);
    println!(
        "read the bodies of the nearest {} systems, to {:.1} ly ({:.1} s)",
        found.len(),
        found.last().map_or(0.0, |each| each.distance_ly),
        start.elapsed().as_secs_f64()
    );

    println!("\nEarth-sized worlds meeting every criterion:");
    for each in &found {
        for (record, misses) in &each.worlds {
            if misses.is_empty() {
                let inputs = coarse_inputs(&record.id)
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"));
                println!(
                    "  {:.1} ly: {}; coarse inputs {inputs:?}",
                    each.distance_ly,
                    world_line(record)
                );
            }
        }
    }
    println!("\nEarth-sized worlds missing one criterion:");
    for each in &found {
        for (record, misses) in &each.worlds {
            if let [miss] = misses.as_slice() {
                println!("  {:.1} ly: {}: {miss}", each.distance_ly, record.id);
            }
        }
    }
    println!("\nRinged gas giants meeting every criterion:");
    for each in &found {
        for (record, rings, misses) in &each.giants {
            if misses.is_empty() {
                println!(
                    "  {:.1} ly: {}",
                    each.distance_ly,
                    giant_line(record, rings)
                );
            }
        }
    }
    let chosen = found.iter().find_map(|each| {
        let (world, giant, rings) = each.both()?;
        println!(
            "\nThe nearest system holding both, {:016x} at {:.1} ly:\n  world {}\n  giant {}",
            each.system,
            each.distance_ly,
            world_line(world),
            giant_line(giant, rings)
        );
        Some((each.system, world.clone(), giant.clone(), rings.to_vec()))
    });

    // The nuclear disc: the layer-C cells about the point one scale length out, and the nearest
    // system there that meets the scene's criteria.
    let target = nuclear_disc_target_ly();
    let mut near_centre = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let offset = [dx, dy, dz].map(|cells| 32.0 * f64::from(cells));
                let position = GalacticPosition::from_light_years(
                    [0, 1, 2].map(|axis| target[axis] + offset[axis]),
                )
                .expect("inside the root cube");
                let key = CellKey::containing(Layer::C, &position).expect("a cell of the grid");
                cell.clear();
                generate_cell(galaxy(), key, &mut cell);
                near_centre.extend(
                    cell.iter()
                        .map(|record| (distance_ly(record, target), *record)),
                );
            }
        }
    }
    near_centre.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.id().raw().cmp(&b.1.id().raw()))
    });
    println!(
        "\nNuclear-disc systems nearest {target:?} ly (one nuclear-disc scale length out), of the \
         {} systems of its 27 cells:",
        near_centre.len()
    );
    let mut nuclear_disc = None;
    let mut shown = 0;
    for (distance, record) in &near_centre {
        if record.population() != Population::NuclearDisc
            || record.existence_at(RECORD_TIME) != Existence::Exists
        {
            continue;
        }
        let system = record.id().raw();
        let answer = summary(&mut first, &universe, system).await;
        let misses = nuclear_disc_misses(record, &answer);
        let (radius, height) = radius_and_height_ly(record);
        let classes = answer
            .stars
            .iter()
            .map(|star| star.class.as_str())
            .collect::<Vec<_>>();
        println!(
            "  {system:016x}: {distance:.1} ly from it, radius {radius:.1} ly, height {height:.1} \
             ly, stars {classes:?}, misses {misses:?}"
        );
        if misses.is_empty() {
            nuclear_disc.get_or_insert(system);
        }
        shown += 1;
        if shown == 8 {
            break;
        }
    }

    println!("\nChosen ({:.1} s):", start.elapsed().as_secs_f64());
    for (system, scene) in [
        (
            chosen.as_ref().map(|(system, ..)| *system),
            "NEAR_SUN_SYSTEM",
        ),
        (nuclear_disc, "NUCLEAR_DISC_SYSTEM"),
    ] {
        let Some(system) = system else {
            println!("  {scene}: none found");
            continue;
        };
        let record = resolve(galaxy(), SystemId::from_raw(system).expect("a system ID"))
            .expect("a found system resolves");
        let (radius, height) = radius_and_height_ly(&record);
        let answer = summary(&mut first, &universe, system).await;
        let star_list = answer
            .stars
            .iter()
            .map(|star| format!("{} {:.4} M☉", star.class, star.mass_msun))
            .collect::<Vec<_>>();
        println!(
            "  {scene} {system:016x}: galactocentric radius {radius:.1} ly, height {height:.1} ly, \
             population {}, age {:.0} Myr, [Fe/H] {:.3}, stars {star_list:?}",
            record.population().name(),
            answer.age_myr,
            answer.fe_h_dex
        );
    }
    if let Some((_, world, giant, rings)) = &chosen {
        for (pin, record, rings) in [
            ("EARTH_SIZED_WORLD", world, &[][..]),
            ("RINGED_GIANT", giant, rings.as_slice()),
        ] {
            println!("  {pin}:");
            for line in pin_figures(record, rings) {
                println!("    {line}");
            }
        }
    }
    first.close().await;
    server.stop().await;
}
