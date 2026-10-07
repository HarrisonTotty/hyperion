//! The sky over a real socket (rendering plan R06, R06.T11.a): the census as bulk jobs, which a
//! range query overtakes and a cancel stops, its answer against the sim's own census, and the
//! fields a request it cannot serve names.
//!
//! Every server here forces its sky's caps to a small radius ([`SkyCaps::forced`]), so that a
//! census near the Sun takes seconds in a test build rather than the thousands of CPU-seconds of
//! one to the derived caps. Most of those seconds are the few cells of layers D and E about the
//! observer, whose every system the census generates there, so a census of more cells runs longer
//! mostly by its jobs' count. Each wait is on what the server reports (its statistics, or the
//! frames it sends), bounded by the harness's patience.

mod common;

use std::num::NonZeroUsize;
use std::sync::OnceLock;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    ConeDto, ErrorCode, EyeDto, GalacticPosition, MAX_SKY_STARS, MassLayer, OpenUniverseRequest,
    RequestBody, RequestError, RequestId, ResponseBody, ServerMessage, SkyGapDto, SkyRequest,
    SkyResponse, SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_server::ServerStats;
use hyperion_server::compute::SkyCaps;
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition as SimPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::id::Layer;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::census::{
    CellOffsets, CensusTallies, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery, census_cell,
    census_plan, merge_census,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::eye_cut;
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::units::{LightYears, Magnitudes};
use tempfile::TempDir;

/// The seed of every universe these tests create.
const SEED: u64 = 0x4d2;

/// Near the Sun's place on the solar circle, ly, as the sim's sky tests stand.
const SUN_LY: [i32; 3] = [0, 26_000, 68];

/// The camera's limit the tests ask, V.
const CAMERA_LIMIT_V: f64 = 9.0;

/// A census that runs to its end within seconds: some 500 cells in 3 jobs near the Sun.
const SMALL_CAP_LY: f64 = 30.0;

/// A census of many jobs, which keeps one worker busy long enough to be overtaken and cancelled:
/// some 22,000 cells in 85 jobs near the Sun, about 35 s of one worker in a test build.
const LONG_CAP_LY: f64 = 120.0;

/// The galaxy of [`SEED`] as the server builds it, with its full potential, built once for the
/// binary.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
}

/// A server whose sky's caps are forced to `radius_ly`, with `workers` CPU workers, and its data
/// directory, which the caller keeps until the server has stopped.
async fn sky_server(radius_ly: f64, workers: usize) -> (TestServer, TempDir) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(NonZeroUsize::new(workers).expect("a worker or more"))
        .sky_caps(SkyCaps::forced(LightYears::new(radius_ly)).expect("a small forced cap"))
        .build();
    (TestServer::start_with(config).await, data_dir)
}

/// A client with a universe of [`SEED`] created and opened, so that its galaxy is warm and a sky
/// asked next finds the pool free for its jobs.
async fn opened(server: &TestServer) -> (TestClient, UniverseIdHex) {
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
    (client, universe)
}

/// A sky near the Sun at the epoch, to [`CAMERA_LIMIT_V`], with no eye.
fn sky(universe: &UniverseIdHex) -> SkyRequest {
    SkyRequest {
        universe: universe.clone(),
        observer: GalacticPosition {
            cell_ly: SUN_LY,
            offset_m: [0.0; 3],
        },
        time: UniverseTime::default(),
        eye: None,
        camera_limit_v: Some(CAMERA_LIMIT_V),
        n_max: None,
        cone: None,
        exclude_system: None,
    }
}

/// The default eye.
fn eye() -> EyeDto {
    EyeDto {
        field_factor: EyeObserver::DEFAULT_FIELD_FACTOR,
        age_years: EyeObserver::DEFAULT_AGE_YEARS,
        pigmentation: EyeObserver::DEFAULT_PIGMENTATION,
    }
}

/// The answer to a sky that must be served.
async fn served(client: &mut TestClient, request: SkyRequest) -> SkyResponse {
    match client.request(RequestBody::Sky(request)).await {
        Ok(ResponseBody::Sky(response)) => *response,
        other => panic!("expected a sky, got {other:?}"),
    }
}

/// The refusal of a sky that must be refused.
async fn refused(client: &mut TestClient, request: SkyRequest) -> RequestError {
    match client.request(RequestBody::Sky(request)).await {
        Err(error) => error,
        Ok(body) => panic!("expected a refusal, got {body:?}"),
    }
}

/// The observer near the Sun at the epoch, as the sim takes it.
fn observer() -> Observer {
    let at = SimPosition::from_light_years(SUN_LY.map(f64::from)).expect("in the cube");
    Observer::new(at, hyperion_sim::time::UniverseTime::EPOCH).expect("an observer")
}

/// The tables a forced census reads, as the server builds them: no star in any table.
fn dark_tables() -> &'static (LuminosityTables, BrightnessEnvelope, CellOffsets) {
    static TABLES: OnceLock<(LuminosityTables, BrightnessEnvelope, CellOffsets)> = OnceLock::new();
    TABLES.get_or_init(|| {
        let galaxy = galaxy();
        (
            LuminosityTables::dark(galaxy),
            BrightnessEnvelope::build(galaxy),
            CellOffsets::build(galaxy),
        )
    })
}

/// A context over [`dark_tables`].
fn context() -> SkyContext<'static> {
    let (tables, envelope, offsets) = dark_tables();
    SkyContext {
        tables,
        envelope,
        offsets,
        noise: NoiseCache::with_capacity(1 << 16),
        cells: &NoSkyCellCache,
        sources: &[],
        modifiers: &NoModifiers,
    }
}

/// The sim's own census of `query` in one pass over its plan's cells, merged as one part.
fn sim_census(query: &SkyQuery) -> SkyCensus {
    let galaxy = galaxy();
    let mut ctx = context();
    let (tables, envelope, _) = dark_tables();
    let mut noise = NoiseCache::with_capacity(1 << 16);
    let plan = census_plan(galaxy, tables, envelope, query, &mut noise);
    let cells: Vec<_> = plan.cells().collect();
    let mut stars = Vec::new();
    let mut tallies: Option<CensusTallies> = None;
    for key in cells {
        let cell = census_cell(galaxy, &mut ctx, key, query, &mut stars);
        match &mut tallies {
            Some(sum) => sum.add(&cell),
            None => tallies = Some(cell),
        }
    }
    merge_census(tallies.map(|tallies| (stars, tallies)), query.n_max())
}

/// The wire's name of each capped layer, in the census's order.
const CAPPED: [(Layer, MassLayer); 6] = [
    (Layer::A, MassLayer::A),
    (Layer::B, MassLayer::B),
    (Layer::C, MassLayer::C),
    (Layer::D, MassLayer::D),
    (Layer::E, MassLayer::E),
    (Layer::BrownDwarf, MassLayer::BrownDwarf),
];

/// A sky near the Sun lists the feature members as not modelled, as every sky does until
/// R06.T16.a (`decision-r06-t16a-scope.md`), and the census its jobs merged is the sim's own, layer
/// by layer.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sky_near_the_sun_lists_feature_members_in_not_modelled() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let response = served(&mut client, sky(&universe)).await;

    assert_eq!(
        response.not_modelled.first(),
        Some(&SkyGapDto::FeatureMembers),
        "{:?}",
        response.not_modelled
    );
    let without_photometry = response
        .census
        .iter()
        .any(|layer| layer.without_photometry > 0);
    assert_eq!(
        response.not_modelled.contains(&SkyGapDto::WhiteDwarfs),
        without_photometry,
        "white dwarfs are not modelled where a census held one: {:?}",
        response.not_modelled
    );

    // The census the jobs merged is the sim's one pass, layer by layer.
    let query = SkyQuery::builder(observer(), Magnitudes::new(CAMERA_LIMIT_V))
        .build()
        .expect("a query")
        .with_caps_forced(LightYears::new(SMALL_CAP_LY))
        .expect("a forced cap");
    let sim = sim_census(&query);
    assert!(!sim.listed().is_empty(), "the Sun's neighbours are listed");
    assert_eq!(response.cut_v.to_bits(), CAMERA_LIMIT_V.to_bits());
    assert_eq!(
        (response.listed, response.overflow),
        (
            u32::try_from(sim.listed().len()).unwrap(),
            u32::try_from(sim.overflow().len()).unwrap()
        )
    );
    assert_eq!(response.census.len(), CAPPED.len());
    for (dto, (layer, wire)) in response.census.iter().zip(CAPPED) {
        let tally = sim.tallies().layer(layer);
        assert_eq!(dto.layer, wire);
        assert_eq!(
            (dto.cap_ly, dto.rule_bound_ly, dto.expected_beyond),
            (SMALL_CAP_LY, SMALL_CAP_LY, 0.0),
            "a forced cap states nothing beyond it"
        );
        assert_eq!(
            (
                u64::from(dto.cells),
                dto.candidates_opened,
                dto.accepted,
                u64::from(dto.listed),
                u64::from(dto.without_photometry)
            ),
            (
                tally.cells(),
                tally.generated(),
                tally.accepted(),
                tally.listed(),
                tally.without_photometry()
            ),
            "{layer:?}"
        );
        assert!(dto.feature_members_absent);
    }
    assert_eq!(
        response
            .census
            .iter()
            .map(|layer| layer.listed)
            .sum::<u32>(),
        response.listed
    );

    // The census's JSON alone until R06.T11.b and T11.c: no payload, band or disc.
    assert_eq!((response.bulk.chunks, response.bulk.bytes), (0, 0));
    assert_eq!((response.stars_bytes, response.band_bytes), (0, 0));
    assert_eq!(response.band.face_texels, 64);
    assert!(response.hosts.is_empty());
    assert_eq!(response.observer, sky(&universe).observer);
    // No listed star lies within a light-year, so the sky holds a Julian year.
    assert_eq!(
        (response.valid_until.seconds, response.valid_until.nanos),
        (31_557_600, 0)
    );

    client.close().await;
    server.stop().await;
}

/// The request's cut is the deeper of the eye's and the camera's: under a shallower camera, the
/// eye's cut, which the server sets from its pre-pass (R06.T9.d), here a dark sky's, since a
/// forced census reads tables that hold no star.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_cut_is_the_eyes_under_a_shallower_camera() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let response = served(
        &mut client,
        SkyRequest {
            eye: Some(eye()),
            camera_limit_v: Some(7.0),
            ..sky(&universe)
        },
    )
    .await;
    let expected = eye_cut(
        galaxy(),
        &mut context(),
        &observer(),
        &EyeObserver::default(),
    );
    assert_eq!(response.cut_v.to_bits(), expected.value().to_bits());
    assert!(
        (8.4..8.6).contains(&response.cut_v),
        "Crumey's darkest limit, 7.99, plus 0.553: {}",
        response.cut_v
    );
    client.close().await;
    server.stop().await;
}

/// Waits until the one worker holds a census job and more wait in the bulk queue, and none in the
/// interactive queue: the sky's census is under way, and nothing else is.
async fn census_under_way(server: &TestServer) -> ServerStats {
    let stats = server
        .stats_until("the sky's census jobs are in the pool", |stats| {
            stats.pool().running() == 1 && stats.pool().queued_bulk() > 0
        })
        .await;
    assert_eq!(
        stats.pool().queued_interactive(),
        0,
        "the census never enters the interactive queue: {:?}",
        stats.pool()
    );
    stats
}

/// A range query sent while a sky's census runs is answered before the sky: the census is bulk
/// work, and the pool's workers take interactive work first.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_range_query_sent_while_a_skys_jobs_run_is_answered_first() {
    let (server, _data_dir) = sky_server(LONG_CAP_LY, 1).await;
    let (mut client, universe) = opened(&server).await;
    let sky_id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    census_under_way(&server).await;

    let query_id = client
        .send_request(RequestBody::SystemsInRange(SystemsInRangeRequest {
            universe: universe.clone(),
            centre: GalacticPosition {
                cell_ly: SUN_LY,
                offset_m: [0.0; 3],
            },
            radius_ly: 10.0,
            time: UniverseTime::default(),
            min_layer: MassLayer::A,
            limit: 100,
            include_stellar: false,
        }))
        .await;
    match client.next_message().await {
        ServerMessage::Response {
            id,
            body: ResponseBody::SystemsInRange(_),
        } => assert_eq!(id, query_id),
        other => panic!("expected the range query's answer before the sky's, got {other:?}"),
    }
    let pool = server.stats().pool();
    assert!(
        pool.running() + pool.queued_bulk() > 0,
        "the sky was still being censused when the query was answered: {pool:?}"
    );

    // The sky is given up rather than waited for.
    client.cancel(sky_id).await;
    expect_cancelled(&mut client, sky_id).await;
    client.close().await;
    server.stop().await;
}

/// Reads the next message, which must be request `id`'s `cancelled`.
async fn expect_cancelled(client: &mut TestClient, id: RequestId) {
    match client.next_message().await {
        ServerMessage::RequestError { id: ended, error } => {
            assert_eq!(ended, id);
            assert_eq!(error.code, ErrorCode::Cancelled);
        }
        other => panic!("expected the sky's `cancelled`, got {other:?}"),
    }
}

/// A cancelled sky's queued census jobs are skipped, the job in hand stops at its next cell, and
/// nothing more is sent for it.
///
/// Once `cancelled` is read the request's token is cancelled, so from then on a job a worker takes
/// is skipped, and at most the one job already in hand completes: the count of completed jobs grows
/// by one at most while the pool drains. Its jobs still queued are skipped, which the pool counts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_sky_stops_its_queued_jobs_and_sends_nothing_further() {
    let (server, _data_dir) = sky_server(LONG_CAP_LY, 1).await;
    let (mut client, universe) = opened(&server).await;
    let sky_id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    let before = census_under_way(&server).await.pool();

    client.cancel(sky_id).await;
    expect_cancelled(&mut client, sky_id).await;
    let at_cancel = server.stats().pool();
    let idle = server
        .stats_until("the cancelled sky's jobs have left the pool", |stats| {
            stats.pool().running() == 0
                && stats.pool().queued_bulk() == 0
                && stats.requests().in_flight() == 0
        })
        .await;
    assert!(
        idle.pool().completed() <= at_cancel.completed() + 1,
        "no job started after the cancel: {at_cancel:?}, then {:?}",
        idle.pool()
    );
    assert!(
        idle.pool().cancelled() > before.cancelled(),
        "the sky's queued jobs were skipped: {before:?}, then {:?}",
        idle.pool()
    );
    assert_eq!(idle.requests().cancelled(), 1);

    // Nothing more comes for the sky: the next message answers this ping.
    match client.ping(9).await {
        ServerMessage::Pong { nonce } => assert_eq!(nonce, 9),
        other => panic!("expected the pong, got {other:?}"),
    }
    client.close().await;
    server.stop().await;
}

/// `n_max` above [`MAX_SKY_STARS`], or zero, is refused naming `n_max`.
#[tokio::test]
async fn n_max_above_the_cap_is_a_bad_request_naming_n_max() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 1).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    for n_max in [MAX_SKY_STARS + 1, 0] {
        let error = refused(
            &mut client,
            SkyRequest {
                n_max: Some(n_max),
                ..sky(&universe)
            },
        )
        .await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("n_max")),
            "{n_max}: {error:?}"
        );
    }
    client.close().await;
    server.stop().await;
}

/// A request with both the eye and a cone is refused naming `cone`: a cone is an instrument's
/// field stop, which the naked eye has none of (`decision-r06-t8k-cone.md`, item 2).
#[tokio::test]
async fn an_eye_with_a_cone_is_a_bad_request_naming_cone() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 1).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    let error = refused(
        &mut client,
        SkyRequest {
            eye: Some(eye()),
            cone: Some(ConeDto {
                axis: [0.0, 0.0, 1.0],
                half_angle_deg: 2.5,
            }),
            ..sky(&universe)
        },
    )
    .await;
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("cone")),
        "{error:?}"
    );
    assert!(
        error.message.contains("field stop"),
        "the refusal says why: {}",
        error.message
    );
    client.close().await;
    server.stop().await;
}
