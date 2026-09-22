//! The range query over a real socket: what it returns, what its census says, and which field a
//! request it cannot serve names (plan 04, P04.T14.d).
//!
//! Every query here is over one fixed seed, so the systems, their IDs and their marks are the ones
//! the sim resolves for that seed: the test builds the same galaxy and checks the wire against it.

mod common;

use std::collections::BTreeMap;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    Census, ErrorCode, GalacticPosition, LayerStatus, MassLayer, RequestBody, RequestError,
    ResponseBody, ServerMessage, SystemRecord, SystemsInRange, SystemsInRangeRequest,
    UniverseIdHex, UniverseTime,
};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::resolve;
use hyperion_sim::id::SystemId;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;

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
    assert_eq!(earlier.systems.len() + dropped, epoch.systems.len());

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
