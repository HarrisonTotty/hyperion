//! Plan 09's feature members in the range query over a real socket (P09.T40.a).
//!
//! With the serve gate open ([`FeatureMembers::Served`]), a query inside a globular returns its
//! members beside the grid's systems, each row with what placed it, a member's with its class and
//! its own velocity, as the sim's range query with plan 09's `FeatureMemberSource` finds them.
//! Behind the gate, the default at the version in force until P09.T2.c's, the same query answers
//! as the grid alone, with no origin on any row, so that no reply moves before then.

mod common;

use std::sync::OnceLock;

use common::origins::{SEED, at_years, galaxy};
use common::{TestClient, TestServer};
use hyperion_protocol::{
    ClassKindWire, FeatureIdHex, GalacticPosition, GenerationWire, MassLayer, MemberClassWire,
    MultiplicityWire, RequestBody, ResponseBody, SystemOriginWire, SystemRecord, SystemsInRange,
    SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_server::FeatureMembers;
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use hyperion_sim::galaxy::features::interior::{ClassKind, Generation, MemberClass, Multiplicity};
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::features::members::{FeatureInterior, KeepInteriors, resolve_member};
use hyperion_sim::galaxy::placement::NoCache;
use hyperion_sim::galaxy::query::{RangeQuery, RangeResult, SystemSource, range_query};
use hyperion_sim::id::{Layer, SystemId, SystemIdKind};
use hyperion_sim::time::UniverseTime as SimTime;
use hyperion_sim::units::LightYears;

/// The query's radius about the globular's centre, ly: enough for some hundreds of its members.
const RADIUS_LY: f64 = 1.0;

/// The interior of the globular with members nearest the Sun-like point.
fn a_globular() -> &'static FeatureInterior {
    static GLOBULAR: OnceLock<FeatureInterior> = OnceLock::new();
    GLOBULAR.get_or_init(|| {
        let sun = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
            .expect("in the root cube");
        let mut globulars: Vec<_> =
            FeatureCatalogue::walk_process(galaxy(), FeatureProcess::Globular).collect();
        globulars.sort_by(|a, b| {
            let away = |f: &hyperion_sim::galaxy::features::catalogue::FeatureRecord| {
                f.position().distance_to(&sun).value()
            };
            away(a).total_cmp(&away(b))
        });
        globulars
            .iter()
            .find_map(|feature| FeatureInterior::of(galaxy(), feature))
            .expect("the galaxy has a globular with members")
    })
}

/// The request for every system within [`RADIUS_LY`] of the globular's centre at `time`.
fn inside_the_globular(universe: &UniverseIdHex, time: UniverseTime) -> RequestBody {
    let centre = a_globular().feature().position();
    RequestBody::SystemsInRange(SystemsInRangeRequest {
        universe: universe.clone(),
        centre: GalacticPosition {
            cell_ly: centre.cell().to_array(),
            offset_m: centre.offset_metres(),
        },
        radius_ly: RADIUS_LY,
        time,
        min_layer: MassLayer::A,
        limit: hyperion_server::limits::MAX_CENSUS_LIMIT,
        include_stellar: false,
        mode: hyperion_protocol::QueryModeDto::Now,
    })
}

/// The answer to `body`.
async fn ask(client: &mut TestClient, body: RequestBody) -> SystemsInRange {
    match client.request(body).await {
        Ok(ResponseBody::SystemsInRange(answer)) => answer,
        other => panic!("expected the systems in range, got {other:?}"),
    }
}

/// The sim's own answer to [`inside_the_globular`] at `years`, through `sources`.
fn sim_answer(years: i64, sources: &[&dyn SystemSource]) -> RangeResult {
    let query = RangeQuery::builder(
        *a_globular().feature().position(),
        LightYears::new(RADIUS_LY),
    )
    .time(SimTime::from_julian_years(years).expect("in the clock window"))
    .limit(
        std::num::NonZeroU32::new(hyperion_server::limits::MAX_CENSUS_LIMIT)
            .expect("the limit is above zero"),
    )
    .cell_budget(hyperion_server::limits::MAX_QUERY_CELLS)
    .build()
    .expect("a query");
    range_query(galaxy(), &mut NoCache::new(), sources, &query)
}

/// The IDs of `answer`'s rows, in its order.
fn ids(answer: &SystemsInRange) -> Vec<String> {
    answer
        .systems
        .iter()
        .map(|row| row.id.as_str().to_owned())
        .collect()
}

/// The IDs of `result`'s systems, in its order, as the wire writes them.
fn sim_ids(result: &RangeResult) -> Vec<String> {
    result
        .systems()
        .iter()
        .map(|hit| {
            hyperion_protocol::SystemIdHex::from_u64(hit.id().raw())
                .as_str()
                .to_owned()
        })
        .collect()
}

/// A velocity's speed, km/s.
fn speed_km_s(velocity: [f64; 3]) -> f64 {
    velocity.iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// The member class `class` as the wire carries it.
fn wire_class(class: &MemberClass) -> MemberClassWire {
    MemberClassWire::Cluster {
        kind: match class.kind {
            ClassKind::Living => ClassKindWire::Living,
            ClassKind::WhiteDwarf => ClassKindWire::WhiteDwarf,
            ClassKind::NeutronStar => ClassKindWire::NeutronStar,
            ClassKind::BlackHole => ClassKindWire::BlackHole,
            ClassKind::Tail => ClassKindWire::Tail,
        },
        band: match class.band.layer() {
            Layer::A => MassLayer::A,
            Layer::B => MassLayer::B,
            Layer::C => MassLayer::C,
            Layer::D => MassLayer::D,
            Layer::E => MassLayer::E,
            other => panic!("a member's band is stellar, not {other:?}"),
        },
        generation: match class.generation {
            Generation::First => GenerationWire::First,
            Generation::Second => GenerationWire::Second,
        },
        multiplicity: match class.multiplicity {
            Multiplicity::Single => MultiplicityWire::Single,
            Multiplicity::Binary => MultiplicityWire::Binary,
        },
    }
}

/// `row` says what placed it: a member of `feature` its feature, its class and its own velocity,
/// its member record's through `interiors`; a grid system `grid`, with no class.
fn assert_row_says_what_placed_it(
    row: &SystemRecord,
    feature: &FeatureIdHex,
    interiors: &KeepInteriors<'_>,
) {
    let id = SystemId::from_raw(row.id.to_u64()).expect("a system ID");
    match id.kind() {
        SystemIdKind::FeatureMember(member_id) => {
            assert_eq!(
                row.origin,
                Some(SystemOriginWire::FeatureMember {
                    feature: feature.clone()
                }),
                "{}",
                row.id.as_str()
            );
            let member =
                resolve_member(galaxy(), interiors, member_id).expect("a member returned resolves");
            assert_eq!(row.member_class, Some(wire_class(member.class())));
            // Its own velocity, its cluster's and its own, not plan 08's draw.
            let own = member.velocity().metres_per_second().map(|v| v / 1_000.0);
            for (wire, sim) in row.velocity_km_s.iter().zip(own) {
                assert!(
                    (wire - sim).abs() <= 1e-9 * sim.abs().max(1.0),
                    "{:?}",
                    row.velocity_km_s
                );
            }
            assert!(
                speed_km_s(row.velocity_km_s) < 1_000.0,
                "{:?}",
                row.velocity_km_s
            );
        }
        SystemIdKind::Grid(_) => {
            assert_eq!(row.origin, Some(SystemOriginWire::Grid));
            assert_eq!(row.member_class, None);
        }
        other => panic!("a row of kind {other:?} inside a globular"),
    }
    assert_eq!(row.supernova, None);
}

/// Each row of `answer` is where the sim's own answer `expected` puts it, bit for bit.
fn assert_positions_are_the_sims(answer: &SystemsInRange, expected: &RangeResult) {
    for (row, hit) in answer.systems.iter().zip(expected.systems()) {
        assert_eq!(row.position.cell_ly, hit.position().cell().to_array());
        assert_eq!(
            row.position.offset_m.map(f64::to_bits),
            hit.position().offset_metres().map(f64::to_bits)
        );
    }
}

/// With the gate open, a query inside a globular returns its members with their origin: every row
/// carries what placed it, each member its feature, its class and its own velocity, and the rows
/// are the sim's own answer with the features' source, at the epoch and a thousand years on, cold
/// and warm alike.
#[tokio::test]
async fn a_query_inside_a_globular_returns_its_members_with_their_origin() {
    let reference = std::thread::spawn(|| {
        a_globular();
    });
    let temp_dir = tempfile::tempdir().expect("a temporary directory");
    let server = TestServer::start_with(
        TestServer::config(temp_dir.path())
            .feature_members(FeatureMembers::Served)
            .build(),
    )
    .await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Omega Reach", SEED).await.id;
    reference.join().expect("the globular is found");
    let interior = a_globular();
    let feature = FeatureIdHex::from_u64(interior.feature().id().object_word());
    // The reference keeps each interior it builds, as the server does, so that it builds each once.
    let interiors = KeepInteriors::new(galaxy());
    let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);

    for years in [0, 1_000] {
        let answer = ask(&mut client, inside_the_globular(&universe, at_years(years))).await;
        let expected = sim_answer(years, &[&source]);
        assert_eq!(ids(&answer), sim_ids(&expected), "at {years} years");
        let members: Vec<_> = answer
            .systems
            .iter()
            .filter(|row| {
                matches!(
                    SystemId::from_raw(row.id.to_u64())
                        .expect("a system ID")
                        .kind(),
                    SystemIdKind::FeatureMember(_)
                )
            })
            .collect();
        assert!(
            members.len() >= 20,
            "{} of {} rows are the globular's members",
            members.len(),
            answer.systems.len()
        );
        for row in &answer.systems {
            assert_row_says_what_placed_it(row, &feature, &interiors);
        }
        assert_positions_are_the_sims(&answer, &expected);
        // Warm, the same answer: the caches hold epoch state alone. (Each query in a globular's
        // core draws every candidate of the nested cells it touches, some seconds in a test build,
        // so the warm query is asked once.)
        if years == 0 {
            let again = ask(&mut client, inside_the_globular(&universe, at_years(years))).await;
            assert_eq!(again, answer, "at {years} years, warm");
        }
    }
    // As the chart asks, with briefs, and observed from a hundred light-years off: the same rows,
    // each member's with its brief and with what the observer receives from it.
    let plain = ask(&mut client, inside_the_globular(&universe, at_years(0))).await;
    let mut as_the_chart = inside_the_globular(&universe, at_years(0));
    if let RequestBody::SystemsInRange(request) = &mut as_the_chart {
        request.include_stellar = true;
        let mut observer = request.centre;
        observer.cell_ly[0] += 100;
        request.mode = hyperion_protocol::QueryModeDto::Observed { observer };
    }
    let charted = ask(&mut client, as_the_chart).await;
    assert_eq!(ids(&charted), ids(&plain));
    for (row, before) in charted.systems.iter().zip(&plain.systems) {
        assert_eq!(row.origin, before.origin, "{}", row.id.as_str());
        assert_eq!(row.member_class, before.member_class);
        assert!(row.observed.is_some(), "{} is observed", row.id.as_str());
        assert!(row.stellar.is_some(), "{} has a brief", row.id.as_str());
    }
    // Near the Sun-like point the rows are the grid's, each saying so.
    let mut sunlike = inside_the_globular(&universe, at_years(0));
    if let RequestBody::SystemsInRange(request) = &mut sunlike {
        request.centre = GalacticPosition {
            cell_ly: [0, 26_000, 0],
            offset_m: [0.0; 3],
        };
        request.radius_ly = 10.0;
    }
    let near_the_sun = ask(&mut client, sunlike).await;
    assert!(
        near_the_sun.systems.len() > 8,
        "{} rows",
        near_the_sun.systems.len()
    );
    for row in &near_the_sun.systems {
        assert!(matches!(
            SystemId::from_raw(row.id.to_u64())
                .expect("a system ID")
                .kind(),
            SystemIdKind::Grid(_)
        ));
        assert_eq!(
            row.origin,
            Some(SystemOriginWire::Grid),
            "{}",
            row.id.as_str()
        );
        assert_eq!(row.member_class, None);
    }
    // Every feature whose grid may meet the padded sphere has its interior built, the globular's
    // among them, and each is built once and kept.
    let stats = server.stats();
    assert!(stats.feature_cells().entries() > 0);
    let models = stats.cluster_models();
    assert!(models.entries() >= 1 && models.hits() > 0, "{models:?}");
    client.close().await;
    server.stop().await;
}

/// Behind the gate, the default at the version in force, the same query answers as the grid
/// alone: no member, no origin on any row and no `origin` key on the wire, so that no reply moves
/// before P09.T2.c (P09.T40.a).
#[tokio::test]
async fn behind_the_gate_a_query_inside_a_globular_answers_as_the_grid() {
    assert_eq!(
        FeatureMembers::default(),
        FeatureMembers::of_version(GENERATOR_VERSION)
    );
    assert_eq!(FeatureMembers::default(), FeatureMembers::Held);
    let reference = std::thread::spawn(|| {
        a_globular();
    });
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Omega Reach", SEED).await.id;
    reference.join().expect("the globular is found");
    let body = inside_the_globular(&universe, at_years(0));
    let id = client.send_request(body).await;
    let frame = client.next_text().await;
    assert!(!frame.contains("\"origin\""), "no row carries an origin");
    assert!(!frame.contains("\"member_class\""));
    let answer = match serde_json::from_str(&frame).expect("a server message") {
        hyperion_protocol::ServerMessage::Response {
            id: answered,
            body: ResponseBody::SystemsInRange(answer),
        } if answered == id => answer,
        other => panic!("expected the systems in range, got {other:?}"),
    };
    assert_eq!(ids(&answer), sim_ids(&sim_answer(0, &[])));
    assert!(answer.systems.iter().all(|row| {
        matches!(
            SystemId::from_raw(row.id.to_u64())
                .expect("a system ID")
                .kind(),
            SystemIdKind::Grid(_)
        ) && row.origin.is_none()
            && row.member_class.is_none()
    }));
    client.close().await;
    server.stop().await;
}
