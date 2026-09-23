//! The range query over a real socket: what it returns, what its census says, and which field a
//! request it cannot serve names (plan 04, P04.T14.d).
//!
//! Every query here is over one fixed seed, so the systems, their IDs and their marks are the ones
//! the sim resolves for that seed: the test builds the same galaxy and checks the wire against it.
//! One answer is also pinned whole, as JSON, in `golden/systems_in_range.golden`.

mod common;

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use common::{TestClient, TestServer, pretty_json_frame};
use hyperion_protocol::{
    Census, ErrorCode, GalacticPosition, LayerStatus, MassLayer, RequestBody, RequestError,
    ResponseBody, ServerMessage, SystemRecord, SystemsInRange, SystemsInRangeRequest,
    UniverseIdHex, UniverseTime,
};
use hyperion_server::limits::MAX_QUERY_CELLS;
use hyperion_sim::coords::LyCell;
use hyperion_sim::galaxy::placement::{NoCache, resolve};
use hyperion_sim::galaxy::query::{MassFloor, RangeQuery, RangeResult, range_query};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{LightYears, SolarMasses};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed of every universe these tests create.
const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year: the wire carries a time as whole seconds.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// The terms of one `systems_in_range` request, so that a test changes only what it means to.
#[derive(Debug, Clone)]
struct Query {
    centre: GalacticPosition,
    radius_ly: f64,
    time: UniverseTime,
    min_layer: MassLayer,
    limit: u32,
}

impl Query {
    /// Fifty light-years of the Sun-like point at the epoch: in the plane, 26,000 ly out on the +y
    /// axis and clear of the bar, with every layer wanted under the largest limit.
    fn sunlike() -> Self {
        Self {
            centre: at_cell([0, 26_000, 0]),
            radius_ly: 50.0,
            time: at_years(0),
            min_layer: MassLayer::A,
            limit: hyperion_server::limits::MAX_CENSUS_LIMIT,
        }
    }

    fn body(&self, universe: &UniverseIdHex) -> RequestBody {
        RequestBody::SystemsInRange(SystemsInRangeRequest {
            universe: universe.clone(),
            centre: self.centre,
            radius_ly: self.radius_ly,
            time: self.time,
            min_layer: self.min_layer,
            limit: self.limit,
            include_stellar: false,
        })
    }
}

/// The centre of a light-year cell, as the wire carries a position.
fn at_cell(cell_ly: [i32; 3]) -> GalacticPosition {
    GalacticPosition {
        cell_ly,
        offset_m: [0.0; 3],
    }
}

/// The instant `years` Julian years from the epoch.
fn at_years(years: i64) -> UniverseTime {
    UniverseTime {
        seconds: years * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    }
}

/// Asks `query` of `universe` and returns the answer, or the error that refused it.
async fn ask(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    query: &Query,
) -> Result<SystemsInRange, RequestError> {
    client
        .request(query.body(universe))
        .await
        .map(|response| match response {
            ResponseBody::SystemsInRange(answer) => answer,
            other => panic!("expected the systems in range, got {other:?}"),
        })
}

/// Asks `query` and expects it to be refused, returning the code and the field named.
async fn refused(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    query: &Query,
) -> (ErrorCode, Option<String>) {
    let error = ask(client, universe, query).await.unwrap_err();
    (error.code, error.field)
}

/// The distance between two positions in light-years, by the wire's own arithmetic: the cells
/// subtract exactly and the metre offsets are a correction well under a light-year (design note
/// 10).
fn distance_ly(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    let squares: f64 = a
        .cell_ly
        .iter()
        .zip(b.cell_ly)
        .zip(a.offset_m.iter().zip(b.offset_m))
        .map(|((&a_cell, b_cell), (&a_offset, b_offset))| {
            let delta = f64::from(a_cell) - f64::from(b_cell)
                + (a_offset - b_offset) / METRES_PER_LIGHT_YEAR;
            delta * delta
        })
        .sum();
    squares.sqrt()
}

/// One layer's line in a census.
fn status(census: &Census, layer: MassLayer) -> LayerStatus {
    census
        .layers
        .iter()
        .find(|line| line.layer == layer)
        .unwrap_or_else(|| panic!("no census line for layer {layer:?}"))
        .status
}

/// The sim's own answer to `query`, run in the test over a galaxy built here and no cache, so that
/// the wire's answer can be held to it field by field.
///
/// The query is built from the request's terms with plan 03's builder directly, not through the
/// server's conversion, and carries the server's cell budget, which the plan fixes (design note 24).
fn sim_answer(galaxy: &Galaxy, query: &Query) -> (RangeQuery, RangeResult) {
    let centre = hyperion_sim::coords::GalacticPosition::new(
        LyCell::new(query.centre.cell_ly),
        query.centre.offset_m,
    )
    .expect("the test's centres are canonical");
    let floor = match query.min_layer {
        MassLayer::A => MassFloor::LayerA,
        MassLayer::B => MassFloor::LayerB,
        MassLayer::C => MassFloor::LayerC,
        MassLayer::D => MassFloor::LayerD,
        MassLayer::E => MassFloor::LayerE,
    };
    let sim_query = RangeQuery::builder(centre, LightYears::new(query.radius_ly))
        .time(
            hyperion_sim::time::UniverseTime::new(query.time.seconds, query.time.nanos)
                .expect("the test's times are well formed"),
        )
        .limit(NonZeroU32::new(query.limit).expect("the test's limits are above zero"))
        .mass_floor(floor)
        .cell_budget(MAX_QUERY_CELLS)
        .build()
        .expect("the test's queries are valid");
    let result = range_query(galaxy, &mut NoCache::new(), &[], &sim_query);
    (sim_query, result)
}

/// The wire's layer for each stellar layer, and the band plan 02 gives it in M☉ of initial mass,
/// written out here rather than read from the server or the sim's layer table.
fn wire_layer_and_band(layer: Layer) -> (MassLayer, f64, f64) {
    match layer {
        Layer::A => (MassLayer::A, 0.08, 0.5),
        Layer::B => (MassLayer::B, 0.5, 0.75),
        Layer::C => (MassLayer::C, 0.75, 2.5),
        Layer::D => (MassLayer::D, 2.5, 8.0),
        Layer::E => (MassLayer::E, 8.0, 150.0),
        Layer::BrownDwarf | Layer::RoguePlanet => panic!("the first milestone places no {layer:?}"),
    }
}

/// The wire's population for each of the sim's.
fn wire_population(population: Population) -> hyperion_protocol::Population {
    match population {
        Population::YoungThinDisc => hyperion_protocol::Population::YoungThinDisc,
        Population::OldThinDisc => hyperion_protocol::Population::OldThinDisc,
        Population::ThickDisc => hyperion_protocol::Population::ThickDisc,
        Population::Bulge => hyperion_protocol::Population::Bulge,
        Population::LongBar => hyperion_protocol::Population::LongBar,
        Population::NuclearDisc => hyperion_protocol::Population::NuclearDisc,
        Population::Halo => hyperion_protocol::Population::Halo,
    }
}

/// Whether `a` and `b` agree to a relative `1e-15`: `serde_json` writes a float that round-trips,
/// but its own parser is exact only with `float_roundtrip`, so the test's reading can be a last
/// bit out.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-15 * a.abs().max(b.abs())
}

/// Holds the wire's answer to the sim's own for the same terms: every record in the same order with
/// the same ID, layer, population, mass, age and position, and every line of the census with plan
/// 02's band, plan 03's expected count, the number of records the answer holds in that layer, and
/// `included` exactly for the layers the census admitted.
fn assert_answer_is_the_sims(
    answer: &SystemsInRange,
    sim_query: &RangeQuery,
    result: &RangeResult,
) {
    let time = sim_query.time();
    assert_eq!(answer.systems.len(), result.systems().len());
    let mut returned = BTreeMap::<MassLayer, u32>::new();
    for (record, hit) in answer.systems.iter().zip(result.systems()) {
        let sim = hit.record();
        let (layer, _, _) = wire_layer_and_band(sim.layer());
        assert_eq!(record.id.to_u64(), sim.id().raw(), "{record:?}");
        assert_eq!(record.designation, sim.id().designation().to_string());
        assert_eq!(record.layer, layer, "{record:?}");
        assert_eq!(
            record.population,
            wire_population(sim.population()),
            "{record:?}"
        );
        assert!(
            close(record.initial_mass_msun, sim.primary_initial_mass().value()),
            "{record:?}"
        );
        let age_myr = sim.age_at(time).value() / 1e6;
        assert!(
            (record.age_myr - age_myr).abs() <= 1e-12 * age_myr.abs().max(1.0),
            "{record:?} is {age_myr} Myr old at the query's time"
        );
        assert_eq!(record.position.cell_ly, hit.position().cell().to_array());
        let moved_m = distance_ly(
            &record.position,
            &GalacticPosition {
                cell_ly: hit.position().cell().to_array(),
                offset_m: hit.position().offset_metres(),
            },
        ) * METRES_PER_LIGHT_YEAR;
        assert!(moved_m < 10.0, "{record:?} is {moved_m} m from the sim's");
        *returned.entry(layer).or_default() += 1;
    }

    let census = result.census();
    assert_eq!(answer.census.limit, sim_query.limit().get());
    assert_eq!(
        answer.census.complete_above_msun,
        census.complete_above().map(SolarMasses::value)
    );
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];
    assert_eq!(answer.census.layers.len(), layers.len());
    for (line, layer) in answer.census.layers.iter().zip(layers) {
        let (wire, lo, hi) = wire_layer_and_band(layer);
        assert_eq!(line.layer, wire, "the census lists A to E in order");
        assert!(
            close(line.mass_min_msun, lo) && close(line.mass_max_msun, hi),
            "{line:?}"
        );
        assert!(
            close(line.expected, census.expected().get(layer)),
            "{line:?}"
        );
        assert_eq!(
            line.returned,
            returned.get(&wire).copied().unwrap_or(0),
            "the census counts what the answer holds: {line:?}"
        );
        assert_eq!(
            line.status == LayerStatus::Included,
            census.layers().contains(layer),
            "{line:?}"
        );
    }
}

/// The records by ID, for comparing two answers.
fn by_id(answer: &SystemsInRange) -> BTreeMap<&str, &SystemRecord> {
    answer
        .systems
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect()
}

#[tokio::test]
async fn every_system_returned_is_inside_the_radius_and_resolves_to_its_own_record() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let query = Query::sunlike();
    let answer = ask(&mut client, &universe.id, &query).await.unwrap();
    assert!(
        !answer.systems.is_empty(),
        "fifty light-years of the solar circle holds thousands of systems"
    );

    // The same galaxy the server built, to resolve what it sent.
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut ids: Vec<&str> = Vec::with_capacity(answer.systems.len());
    for record in &answer.systems {
        let id = record.id.as_str();
        assert_eq!(id.len(), 16, "{id} is not sixteen hexadecimal digits");
        assert!(
            id.bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{id} is not lower-case hexadecimal"
        );
        ids.push(id);
        assert!(
            distance_ly(&record.position, &query.centre) <= query.radius_ly,
            "{record:?} lies outside the radius"
        );
        // The ID resolves, without generating its cell, to the very system that was sent.
        let system = SystemId::from_raw(record.id.to_u64()).expect("a record carries a system ID");
        let resolved = resolve(&galaxy, system).expect("a system the query returned resolves");
        assert_eq!(record.designation, system.designation().to_string());
        assert_eq!(record.id.to_u64(), resolved.id().raw());
        // To a relative 10⁻¹⁵ rather than to the bit, because it is this client that rounds:
        // `serde_json` writes a float that round-trips, but its own parser is only exact with the
        // `float_roundtrip` feature, and so can be a last bit out. A browser's `JSON.parse` is
        // correctly rounded, so the client the wire is for reads the value the server sent.
        let mass = resolved.primary_initial_mass().value();
        assert!(
            (record.initial_mass_msun - mass).abs() <= 1e-15 * mass,
            "{record:?} was placed at {mass} M☉"
        );
        // At the epoch a system is where it was placed, until plan 08 draws the velocities. The
        // query moves every system by its velocity, zero here, and a position rebuilt that way can
        // differ by the last bit of an offset word, which is about two metres at 8 × 10¹⁵ m.
        let placed = GalacticPosition {
            cell_ly: resolved.epoch_position().cell().to_array(),
            offset_m: resolved.epoch_position().offset_metres(),
        };
        let moved_m = distance_ly(&record.position, &placed) * METRES_PER_LIGHT_YEAR;
        assert!(
            moved_m < 10.0,
            "{record:?} sits {moved_m} m from where it was placed"
        );
    }
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "two records share an ID");

    // And the whole answer is the sim's own for the same terms, census and all.
    let (sim_query, result) = sim_answer(&galaxy, &query);
    assert_answer_is_the_sims(&answer, &sim_query, &result);

    client.close().await;
    server.stop().await;
}

/// The answer on the wire to one fixed query, pinned under the generator version (plan 04's Risks:
/// T14.e compares a cold answer with a cold one, which cannot see an answer that moves the same way
/// on every run).
///
/// The query is forty light-years of the Sun-like point, 250 years before the epoch, under a limit
/// of 100 expected systems. Layers E and D fit under it, about 33 systems expected, and C does not,
/// with about 150 more, so the answer holds a few dozen systems from two layers, and the census
/// pins both kinds of line, included and over the limit, with every layer's expected count. The
/// records pin their order, IDs, positions, masses and populations, and their ages at a time that is
/// not the epoch. Fifty light-years at the default limit, the other tests' sphere, holds about 2,000
/// systems, too many to review.
///
/// The golden is the frame as it arrived, laid out as pretty JSON so that it can be reviewed but
/// with every number copied byte for byte (`common::pretty_json_frame`). It is not written from the
/// parsed answer, because `serde_json` without `float_roundtrip` reads some of these floats a last
/// bit out: 16 of this answer's 185 (13 offsets and 3 ages) do not survive a parse and a print, so
/// a golden of the parse could not see the server move one of them by an ulp.
#[tokio::test]
async fn the_answer_to_a_fixed_query_is_the_golden_response() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let mut query = Query::sunlike();
    query.radius_ly = 40.0;
    query.time = at_years(-250);
    query.limit = 100;
    let id = client.send_request(query.body(&universe.id)).await;
    let frame = client.next_text().await;
    let message: ServerMessage = serde_json::from_str(&frame).unwrap();
    // The layout is `serde_json`'s own, which the re-indenter must reproduce: on the parse printed
    // compactly, it gives exactly what `to_string_pretty` gives.
    assert_eq!(
        pretty_json_frame(&serde_json::to_string(&message).unwrap()),
        serde_json::to_string_pretty(&message).unwrap()
    );
    let answer = match message {
        ServerMessage::Response {
            id: answered,
            body: ResponseBody::SystemsInRange(answer),
        } if answered == id => answer,
        other => panic!("expected the systems in range, got {other:?}"),
    };

    // What the golden is for: a census stopped by the limit, with more than one layer in it, and an
    // answer short enough to review.
    let included: Vec<MassLayer> = answer
        .census
        .layers
        .iter()
        .filter(|line| line.status == LayerStatus::Included)
        .map(|line| line.layer)
        .collect();
    assert_eq!(
        included,
        [MassLayer::D, MassLayer::E],
        "{:?}",
        answer.census
    );
    assert_eq!(status(&answer.census, MassLayer::C), LayerStatus::OverLimit);
    assert!(
        (20..=60).contains(&answer.systems.len()),
        "{} systems",
        answer.systems.len()
    );
    for layer in [MassLayer::D, MassLayer::E] {
        assert!(
            answer.systems.iter().any(|record| record.layer == layer),
            "no system of layer {layer:?}"
        );
    }
    // And it is the sim's own answer, so the golden pins what the sim and the wire agree on.
    let (sim_query, result) = sim_answer(&Galaxy::new(Seed::new(SEED)), &query);
    assert_answer_is_the_sims(&answer, &sim_query, &result);

    // The frame the client received, envelope and all, under the golden header that ties it to the
    // generator version: a version bump re-blesses this file.
    let mut golden = GoldenWriter::new();
    golden.header(GENERATOR_VERSION.get());
    for line in pretty_json_frame(&frame).lines() {
        golden.line(line);
    }
    golden!("systems_in_range", &golden.finish());

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn the_answer_echoes_the_request_and_repeats_itself_from_the_cache() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let mut query = Query::sunlike();
    query.time = at_years(-250);
    let first = ask(&mut client, &universe.id, &query).await.unwrap();
    assert_eq!(first.universe, universe.id);
    assert_eq!(first.centre, query.centre);
    assert_eq!(first.radius_ly.to_bits(), query.radius_ly.to_bits());
    assert_eq!(first.time, query.time);
    assert_eq!(first.census.limit, query.limit);
    let cells = server.stats().cells();
    assert!(
        cells.entries() > 0,
        "the query's cells were kept: {cells:?}"
    );

    // The same request twice is the same JSON, byte for byte, warm or cold.
    let again = ask(&mut client, &universe.id, &query).await.unwrap();
    assert_eq!(
        serde_json::to_string(&again).unwrap(),
        serde_json::to_string(&first).unwrap()
    );
    let cells = server.stats().cells();
    assert!(
        cells.hits() > 0,
        "the second query read the first one's cells: {cells:?}"
    );

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_query_before_the_epoch_drops_only_the_systems_not_yet_born() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let epoch = ask(&mut client, &universe.id, &Query::sunlike())
        .await
        .unwrap();
    let mut earlier_query = Query::sunlike();
    earlier_query.time = at_years(-500);
    let earlier = ask(&mut client, &universe.id, &earlier_query)
        .await
        .unwrap();
    assert!(!earlier.systems.is_empty());
    assert_eq!(
        earlier.census.complete_above_msun, epoch.census.complete_above_msun,
        "the census does not depend on the time asked about"
    );

    // Five hundred years is 0.0005 Myr, and no system has moved: plan 08 draws the velocities, and
    // until then a position is the position at the epoch (this test changes with that plan).
    let five_hundred_years_myr = 0.0005;
    let then = by_id(&earlier);
    let mut dropped = 0;
    for record in &epoch.systems {
        let born_by_then = record.age_myr > five_hundred_years_myr;
        let Some(earlier_record) = then.get(record.id.as_str()) else {
            assert!(
                !born_by_then,
                "{record:?} was born before the query's time and is missing from it"
            );
            dropped += 1;
            continue;
        };
        assert!(
            born_by_then,
            "{record:?} was not born yet at the query's time"
        );
        assert_eq!(earlier_record.position, record.position);
        assert_eq!(earlier_record.layer, record.layer);
        assert_eq!(earlier_record.population, record.population);
        let younger = record.age_myr - earlier_record.age_myr;
        assert!(
            (younger - five_hundred_years_myr).abs() < 1e-9,
            "{younger} Myr younger, not {five_hundred_years_myr}: {earlier_record:?}"
        );
    }
    // Nothing is dropped here, and nothing can be: the youngest of these systems is 0.71 Myr old
    // (generator version 8), and a system under the clock window's 1,000 years would take a sphere
    // of about a million, fifty times the largest census. The drop itself is plan 03's, pinned by
    // `motion_drops_a_system_that_is_not_born_yet`; what the wire adds is the time, which the ages
    // above and the comparison with the sim below hold.
    assert_eq!(
        dropped, 0,
        "{dropped} systems were born within five centuries of the epoch"
    );
    assert_eq!(earlier.systems.len(), epoch.systems.len());

    // Both answers are the sim's own, ages at the query's time included.
    let galaxy = Galaxy::new(Seed::new(SEED));
    for (answer, query) in [(&epoch, Query::sunlike()), (&earlier, earlier_query)] {
        let (sim_query, result) = sim_answer(&galaxy, &query);
        assert_answer_is_the_sims(answer, &sim_query, &result);
    }

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_mass_floor_leaves_the_lighter_layers_out_and_says_so() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let mut query = Query::sunlike();
    query.min_layer = MassLayer::C;
    let answer = ask(&mut client, &universe.id, &query).await.unwrap();

    for layer in [MassLayer::A, MassLayer::B] {
        assert_eq!(status(&answer.census, layer), LayerStatus::BelowMassFloor);
    }
    for layer in [MassLayer::C, MassLayer::D, MassLayer::E] {
        assert_eq!(status(&answer.census, layer), LayerStatus::Included);
    }
    // 0.75 M☉ is the lower edge of layer C's band (plan 02's mass bands).
    assert_eq!(answer.census.complete_above_msun, Some(0.75));
    assert!(!answer.systems.is_empty());
    for record in &answer.systems {
        assert!(
            matches!(record.layer, MassLayer::C | MassLayer::D | MassLayer::E),
            "{record:?}"
        );
    }
    // Every layer's expected count is reported, whether it is in the answer or not.
    for line in &answer.census.layers {
        assert!(line.expected > 0.0, "{line:?}");
        if line.status != LayerStatus::Included {
            assert_eq!(line.returned, 0, "{line:?}");
        }
    }
    let (sim_query, result) = sim_answer(&Galaxy::new(Seed::new(SEED)), &query);
    assert_answer_is_the_sims(&answer, &sim_query, &result);

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_query_over_the_limit_returns_whole_layers_or_none() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    // Fifty light-years of the galactic centre expects tens of thousands of systems in layer E
    // alone, so nothing fits under a limit of 5,000: a valid answer with no systems, not an error
    // (design note 13).
    let mut query = Query::sunlike();
    query.centre = at_cell([0, 0, 0]);
    query.limit = 5_000;
    let answer = ask(&mut client, &universe.id, &query).await.unwrap();

    assert_eq!(answer.census.limit, 5_000);
    assert_eq!(answer.census.complete_above_msun, None);
    assert!(answer.systems.is_empty());
    for line in &answer.census.layers {
        assert_eq!(line.status, LayerStatus::OverLimit, "{line:?}");
        assert_eq!(line.returned, 0, "no layer is partial: {line:?}");
        assert!(line.expected > f64::from(query.limit), "{line:?}");
    }
    assert_eq!(
        server.stats().cells().entries(),
        0,
        "a census that admits nothing generates nothing"
    );

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_query_over_the_cell_budget_still_answers() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    // Five thousand light-years of the halo, 30,000 ly above the centre: layer E alone would take
    // more than the 262,144 cells one query may visit, so every layer is out for the budget and the
    // answer says which rule stopped it (design note 24).
    let mut query = Query::sunlike();
    query.centre = at_cell([0, 0, 30_000]);
    query.radius_ly = 5_000.0;
    let answer = ask(&mut client, &universe.id, &query).await.unwrap();

    assert_eq!(answer.centre, query.centre);
    assert_eq!(answer.census.complete_above_msun, None);
    assert!(answer.systems.is_empty());
    for line in &answer.census.layers {
        assert_eq!(line.status, LayerStatus::OverCellBudget, "{line:?}");
        assert_eq!(line.returned, 0, "{line:?}");
    }
    // The connection is healthy afterwards, and a smaller sphere at the same point answers with
    // systems in it.
    query.radius_ly = 200.0;
    let smaller = ask(&mut client, &universe.id, &query).await.unwrap();
    assert!(
        smaller
            .census
            .layers
            .iter()
            .any(|line| line.status == LayerStatus::Included),
        "{:?}",
        smaller.census
    );

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn each_field_it_cannot_use_is_a_bad_request_naming_that_field() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;

    // A radius wider than the galaxy's root cube.
    let mut wide = Query::sunlike();
    wide.radius_ly = 200_000.0;
    assert_eq!(
        refused(&mut client, &universe.id, &wide).await,
        (ErrorCode::BadRequest, Some("radius_ly".to_owned()))
    );
    // A centre outside the root cube.
    let mut outside = Query::sunlike();
    outside.centre = at_cell([65_536, 0, 0]);
    assert_eq!(
        refused(&mut client, &universe.id, &outside).await,
        (ErrorCode::BadRequest, Some("centre".to_owned()))
    );
    // A nanosecond field that is not below a second, and a time outside the clock window.
    let mut nanos = Query::sunlike();
    nanos.time = UniverseTime {
        seconds: 0,
        nanos: 1_000_000_000,
    };
    assert_eq!(
        refused(&mut client, &universe.id, &nanos).await,
        (ErrorCode::BadRequest, Some("time".to_owned()))
    );
    let mut late = Query::sunlike();
    late.time = at_years(2_000);
    assert_eq!(
        refused(&mut client, &universe.id, &late).await,
        (ErrorCode::BadRequest, Some("time".to_owned()))
    );
    // A limit of no systems at all, and one beyond what a response may hold.
    for limit in [0, hyperion_server::limits::MAX_CENSUS_LIMIT + 1] {
        let mut refused_limit = Query::sunlike();
        refused_limit.limit = limit;
        assert_eq!(
            refused(&mut client, &universe.id, &refused_limit).await,
            (ErrorCode::BadRequest, Some("limit".to_owned())),
            "{limit}"
        );
    }
    // Nothing was generated for any of them: each was refused before the galaxy was touched.
    let stats = server.stats();
    assert_eq!((stats.cells().entries(), stats.galaxies().builds()), (0, 0));

    client.close().await;
    server.stop().await;
}

/// One way a request can be wrong, with the code and the field it is refused with.
type Fault = (fn(&mut Query), ErrorCode, &'static str);

/// A request wrong in several ways is refused for the first of them in design note 24's order:
/// universe, time, centre, radius, limit.
///
/// A handler that checked the fields in another order would still name the right field for each
/// fault alone, so every pair of faults is tried, and then all five at once, mended one at a time
/// from the front.
#[tokio::test]
async fn a_request_wrong_in_several_ways_is_refused_for_the_first_in_design_note_24s_order() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    // Not an ID `test_entropy` draws, so no universe has it.
    let unknown = UniverseIdHex::from_u64(0x0bad_0000_0000_0001);
    // Each fault, in design note 24's order.
    let faults: [Fault; 5] = [
        (|_| (), ErrorCode::UnknownUniverse, "universe"),
        (|q| q.time = at_years(2_000), ErrorCode::BadRequest, "time"),
        (
            |q| q.centre = at_cell([65_536, 0, 0]),
            ErrorCode::BadRequest,
            "centre",
        ),
        (
            |q| q.radius_ly = 200_000.0,
            ErrorCode::BadRequest,
            "radius_ly",
        ),
        (|q| q.limit = 0, ErrorCode::BadRequest, "limit"),
    ];
    let ask_with = |faulty: &[usize]| {
        let mut query = Query::sunlike();
        for &index in faulty {
            faults[index].0(&mut query);
        }
        let target = if faulty.contains(&0) {
            &unknown
        } else {
            &universe.id
        };
        (query, target.clone())
    };

    for first in 0..faults.len() {
        for second in first + 1..faults.len() {
            let (query, target) = ask_with(&[first, second]);
            let error = ask(&mut client, &target, &query).await.unwrap_err();
            let (_, code, field) = faults[first];
            assert_eq!(
                (error.code, error.field.as_deref()),
                (code, Some(field)),
                "faults {first} and {second} together are refused for fault {first}: {error:?}"
            );
        }
    }
    let mut remaining: Vec<usize> = (0..faults.len()).collect();
    while let Some(&first) = remaining.first() {
        let (query, target) = ask_with(&remaining);
        let error = ask(&mut client, &target, &query).await.unwrap_err();
        let (_, code, field) = faults[first];
        assert_eq!(
            (error.code, error.field.as_deref()),
            (code, Some(field)),
            "faults {remaining:?} are refused for fault {first}: {error:?}"
        );
        remaining.remove(0);
    }
    // With every fault mended the request is served.
    assert!(
        !ask(&mut client, &universe.id, &Query::sunlike())
            .await
            .unwrap()
            .systems
            .is_empty()
    );

    client.close().await;
    server.stop().await;
}

/// A centre whose offset is not canonical and a radius of zero or below each name their field, and
/// the edges of every range are where design note 24 puts them: the widest radius is served and the
/// next float up is not, both ends of the clock window are inside it and a nanosecond beyond either
/// is outside, and a limit of one system is served.
#[tokio::test]
async fn the_edges_of_every_range_are_where_design_note_24_puts_them() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let tiny = || {
        let mut query = Query::sunlike();
        query.radius_ly = 1.0;
        query
    };

    for offset_m in [[-1.0, 0.0, 0.0], [0.0, METRES_PER_LIGHT_YEAR, 0.0]] {
        let mut query = tiny();
        query.centre.offset_m = offset_m;
        assert_eq!(
            refused(&mut client, &universe.id, &query).await,
            (ErrorCode::BadRequest, Some("centre".to_owned())),
            "{offset_m:?}"
        );
    }
    for radius_ly in [0.0, -1.0, -0.0] {
        let mut query = tiny();
        query.radius_ly = radius_ly;
        assert_eq!(
            refused(&mut client, &universe.id, &query).await,
            (ErrorCode::BadRequest, Some("radius_ly".to_owned())),
            "{radius_ly}"
        );
    }
    // The widest radius is served, from the census alone: nothing that wide fits any limit.
    let mut widest = tiny();
    widest.radius_ly = hyperion_server::limits::MAX_QUERY_RADIUS_LY;
    let answer = ask(&mut client, &universe.id, &widest).await.unwrap();
    assert!(answer.systems.is_empty(), "{:?}", answer.census);
    widest.radius_ly = widest.radius_ly.next_up();
    assert_eq!(
        refused(&mut client, &universe.id, &widest).await,
        (ErrorCode::BadRequest, Some("radius_ly".to_owned()))
    );

    // The clock window is inclusive at both ends, and a nanosecond past either end is outside it.
    let h_seconds = 1_000 * SECONDS_PER_JULIAN_YEAR;
    for (seconds, nanos, inside) in [
        (h_seconds, 0, true),
        (-h_seconds, 0, true),
        (h_seconds, 1, false),
        (-h_seconds - 1, 999_999_999, false),
    ] {
        let mut query = tiny();
        query.time = UniverseTime { seconds, nanos };
        let answer = ask(&mut client, &universe.id, &query).await;
        match (inside, answer) {
            (true, Ok(answer)) => assert_eq!(answer.time, query.time),
            (false, Err(error)) => {
                assert_eq!(
                    (error.code, error.field.as_deref()),
                    (ErrorCode::BadRequest, Some("time"))
                );
            }
            (_, other) => panic!("{seconds} s {nanos} ns: {other:?}"),
        }
    }
    // A limit of one system is a valid query; the census decides what fits.
    let mut one = tiny();
    one.limit = 1;
    assert_eq!(
        ask(&mut client, &universe.id, &one)
            .await
            .unwrap()
            .census
            .limit,
        1
    );

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_radius_that_is_not_a_number_never_reaches_the_query() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;

    // JSON carries no NaN and no infinity, so `1e999` is the nearest a client can come to one. It
    // is not a number JSON holds at all: the frame does not parse, and the lenient probe of design
    // note 4 cannot read its ID either, since that too is a JSON parse. The answer is therefore the
    // connection-level `error`, not a `request_error`, and no ID is freed or ended by it.
    let frame = format!(
        r#"{{"type":"request","id":900,"body":{{"kind":"systems_in_range","universe":"{}",
"centre":{{"cell_ly":[0,26000,0],"offset_m":[0.0,0.0,0.0]}},"radius_ly":1e999,
"time":{{"seconds":0,"nanos":0}},"min_layer":"a","limit":100}}}}"#,
        universe.id.as_str()
    );
    client.send_raw(&frame).await;
    match client.next_message().await {
        ServerMessage::Error { message } => {
            assert!(message.contains("number out of range"), "{message}");
        }
        other => panic!("expected the connection's error, got {other:?}"),
    }
    // The largest radius JSON can carry is a number, and that one names its field.
    let mut huge = Query::sunlike();
    huge.radius_ly = f64::MAX;
    assert_eq!(
        refused(&mut client, &universe.id, &huge).await,
        (ErrorCode::BadRequest, Some("radius_ly".to_owned()))
    );
    // The connection survives both and serves the next query.
    let answer = ask(&mut client, &universe.id, &Query::sunlike())
        .await
        .unwrap();
    assert!(!answer.systems.is_empty());

    client.close().await;
    server.stop().await;
}
