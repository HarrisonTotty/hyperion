//! The sky over a real socket (rendering plan R06, R06.T11.a–c): the census as bulk jobs, which a
//! range query overtakes and a cancel stops, its answer against the sim's own census, its stars
//! and band in R03's bulk frames as the manifest states them, the stars, texels and host discs
//! against the sim's for the same query, its census's cells served again from the server's cache,
//! the galaxy's tables built once for every sky of it, the fields a request it cannot serve names,
//! and the landing switch, off by default, under which `sky` is `unsupported`.
//!
//! Every server here turns the sky on ([`SkyService::Served`]) but the switch's own test, and
//! forces its sky's caps to a small radius ([`SkyCaps::forced`]), so that a census near the Sun
//! takes seconds in a test build rather than the thousands of CPU-seconds of one to the derived
//! caps. Most of those seconds are the few cells of layers D and E about the observer, whose every
//! system the census generates there, so a census of more cells runs longer mostly by its jobs'
//! count. A served sky also marches its band, 24,576 rays, some 27 CPU-s in a test build. Forced
//! caps read tables that hold no star, but for the test of the band's light, which builds the
//! galaxy's own on the pool. Each wait is on what the server reports (its statistics, or the
//! frames it sends), bounded by the harness's patience.

mod common;

use std::num::NonZeroUsize;
use std::sync::OnceLock;

use common::{BinaryHeader, Frame, TestClient, TestServer};
use hyperion_protocol::{
    ConeDto, ErrorCode, EyeDto, GalacticPosition, HostDiscDto, MAX_SKY_STARS, MassLayer,
    OpenUniverseRequest, RequestBody, RequestError, RequestId, ResponseBody, SKY_STAR_BYTES,
    SKY_TEXEL_BYTES, ServerMessage, SkyGapDto, SkyRequest, SkyResponse, SystemIdHex,
    SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_server::compute::SkyCaps;
use hyperion_server::limits::MAX_BINARY_FRAME_BYTES;
use hyperion_server::{ServerStats, SkyService};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition as SimPosition, UnitVector};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{BandSpec, BandTexel, CompleteTo, CubeFace, march_rows, sum_rows};
use hyperion_sim::sky::census::{
    CellOffsets, CensusPlan, CensusTallies, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery,
    SkyStar, census_cell, census_plan, merge_census,
};
use hyperion_sim::sky::disc::{HostDisc, host_discs};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::{Glare, eye_cut, eye_offsets, limit_map};
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::tables::star_colour::LUMINANCE_RGB;
use hyperion_sim::units::{CandelasPerSquareMetre, LightYears, Magnitudes};
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

/// A server that serves the sky, its caps forced to `radius_ly` over tables that hold no star,
/// with `workers` CPU workers, and its data directory, which the caller keeps until the server has
/// stopped.
async fn sky_server(radius_ly: f64, workers: usize) -> (TestServer, TempDir) {
    let caps = SkyCaps::forced(LightYears::new(radius_ly)).expect("a small forced cap");
    sky_server_with(caps, workers).await
}

/// A server that serves the sky to `caps`, with `workers` CPU workers, and its data directory.
async fn sky_server_with(caps: SkyCaps, workers: usize) -> (TestServer, TempDir) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(NonZeroUsize::new(workers).expect("a worker or more"))
        .sky_caps(caps)
        .sky_service(SkyService::Served)
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

/// A sky's answer as the server sent it: the request's ID, its bulk frames in the order they
/// arrived, and the terminal response after them.
struct Sent {
    id: RequestId,
    frames: Vec<(BinaryHeader, Vec<u8>)>,
    response: SkyResponse,
}

impl Sent {
    /// The payload, the frames' payloads joined in the order they arrived.
    fn payload(&self) -> Vec<u8> {
        self.frames
            .iter()
            .flat_map(|(_, payload)| payload.iter().copied())
            .collect()
    }
}

/// The answer to a sky that must be served: its chunks, then its response.
async fn served(client: &mut TestClient, request: SkyRequest) -> Sent {
    let id = client.send_request(RequestBody::Sky(request)).await;
    answer_to(client, id).await
}

/// The answer to the sky `id`, which the client sent and must be served: its chunks, then its
/// response.
async fn answer_to(client: &mut TestClient, id: RequestId) -> Sent {
    let mut frames = Vec::new();
    loop {
        match client.next_frame().await {
            Frame::Binary(header, payload) => frames.push((header, payload)),
            Frame::Message(ServerMessage::Response {
                id: answered,
                body: ResponseBody::Sky(response),
            }) if answered == id => {
                return Sent {
                    id,
                    frames,
                    response: *response,
                };
            }
            Frame::Message(other) => {
                panic!("expected the sky's chunks and then its response, got {other:?}")
            }
        }
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

/// What a sky's jobs read: the luminosity tables, the envelope and the cells' offset bounds.
type Tables = (LuminosityTables, BrightnessEnvelope, CellOffsets);

/// `tables` of [`galaxy`], with its envelope and offset bounds.
fn tables_of(tables: LuminosityTables) -> Tables {
    let galaxy = galaxy();
    (
        tables,
        BrightnessEnvelope::build(galaxy),
        CellOffsets::build(galaxy),
    )
}

/// The tables a forced census reads, as the server builds them: no star in any table.
fn dark_tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| tables_of(LuminosityTables::dark(galaxy())))
}

/// A context over [`dark_tables`].
fn context() -> SkyContext<'static> {
    context_over(dark_tables())
}

/// A context over `tables`.
fn context_over(tables: &Tables) -> SkyContext<'_> {
    let (tables, envelope, offsets) = tables;
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
    sim_census_over(dark_tables(), query).1
}

/// The sim's own plan and census of `query` over `tables`, the census in one pass over the plan's
/// cells, merged as one part.
fn sim_census_over(tables: &Tables, query: &SkyQuery) -> (CensusPlan, SkyCensus) {
    let galaxy = galaxy();
    let mut ctx = context_over(tables);
    let mut noise = NoiseCache::with_capacity(1 << 16);
    let plan = census_plan(galaxy, &tables.0, &tables.1, query, &mut noise);
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
    let census = merge_census(tallies.map(|tallies| (stars, tallies)), query.n_max());
    (plan, census)
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
    let response = served(&mut client, sky(&universe)).await.response;

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

    // The stars and the band in bulk (R06.T11.b and T11.c), and no disc with no system left out.
    assert_eq!(
        (response.stars_bytes, response.band_bytes),
        (u64::from(response.listed) * star_bytes(), band_bytes())
    );
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
    .await
    .response;
    let expected = eye_cut(
        galaxy(),
        &mut context(),
        &observer(),
        &EyeObserver::default(),
        None,
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

/// The bytes of one listed star on the wire (Design note 17).
fn star_bytes() -> u64 {
    u64::try_from(SKY_STAR_BYTES).expect("24 bytes")
}

/// The texels of the server's band: six faces of 64² (Design note 14).
const BAND_TEXELS: usize = 6 * 64 * 64;

/// The bytes of the server's band on the wire: [`BAND_TEXELS`] of 12 bytes (Design note 17).
fn band_bytes() -> u64 {
    u64::try_from(BAND_TEXELS * SKY_TEXEL_BYTES).expect("288 KiB")
}

/// A listed star's apparent V and distance as the wire carries them (Design note 17): bytes 16–17
/// as millimagnitudes and 12–15 as light-years.
fn wire_v_and_distance(star: &[u8; SKY_STAR_BYTES]) -> (i16, f32) {
    (
        i16::from_le_bytes([star[16], star[17]]),
        f32::from_le_bytes(star[12..16].try_into().expect("4 bytes")),
    )
}

/// A sky's manifest is what was sent: its chunks in order before the response, each a frame of at
/// most 256 KiB for the request, their bytes the manifest's, split into the listed stars and the
/// band, the stars the sim's census in its order. The same sky asked again is served from the cell
/// cache, with the same bytes (Design note 12).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_skys_manifest_matches_what_was_sent() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let sent = served(&mut client, sky(&universe)).await;
    let response = &sent.response;

    let manifest = &response.bulk;
    assert!(manifest.chunks > 0, "the Sun's neighbours are sent");
    assert_eq!(sent.frames.len(), usize::try_from(manifest.chunks).unwrap());
    for (index, (header, payload)) in sent.frames.iter().enumerate() {
        assert_eq!(
            (header.request, header.index, header.count),
            (sent.id.0, u32::try_from(index).unwrap(), manifest.chunks),
            "frame {index}"
        );
        assert!(
            payload.len() + 24 <= MAX_BINARY_FRAME_BYTES,
            "frame {index}: {}",
            payload.len()
        );
    }
    let payload = sent.payload();
    assert_eq!(u64::try_from(payload.len()).unwrap(), manifest.bytes);
    assert_eq!(response.stars_bytes + response.band_bytes, manifest.bytes);
    assert_eq!(
        (response.stars_bytes, response.band_bytes),
        (u64::from(response.listed) * star_bytes(), band_bytes()),
        "the stars, then the band"
    );

    // The stars are the sim's census's listed stars, in its order.
    let query = SkyQuery::builder(observer(), Magnitudes::new(CAMERA_LIMIT_V))
        .build()
        .expect("a query")
        .with_caps_forced(LightYears::new(SMALL_CAP_LY))
        .expect("a forced cap");
    let sim = sim_census(&query);
    let stars_bytes = usize::try_from(response.stars_bytes).unwrap();
    let (stars, rest) = payload[..stars_bytes].as_chunks::<SKY_STAR_BYTES>();
    assert!(rest.is_empty(), "whole stars");
    assert_eq!(sim.listed().len(), stars.len());
    for (index, (star, bytes)) in sim.listed().iter().zip(stars).enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the wire's V and distance, as Design note 17 rounds them"
        )]
        let expected = (
            (star.v().value() * 1_000.0).round() as i16,
            star.distance().value() as f32,
        );
        assert_eq!(wire_v_and_distance(bytes), expected, "star {index}");
    }

    // Asked again, every cell the census looked up is served from the cache, with the same bytes.
    let cold = server.stats().sky_cells();
    let lookups = cold.cache().hits() + cold.cache().misses();
    assert!(lookups > 0, "{cold:?}");
    let again = served(&mut client, sky(&universe)).await;
    let warm = server.stats().sky_cells();
    assert_eq!(again.payload(), payload);
    assert_eq!(again.response.bulk, response.bulk);
    assert_eq!(
        (
            warm.cache().hits() - cold.cache().hits(),
            warm.cache().misses() - cold.cache().misses(),
            warm.rebuilt(),
            warm.cache().evictions()
        ),
        (lookups, 0, 0, 0),
        "the second census's cells are the first's: {cold:?}, then {warm:?}"
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

/// With its landing switch off, as it is by default until R06.T8.g, the server answers `sky` as it
/// did before R06.T11.a, `unsupported`, and no job of it reaches the pool (R06.T11.c).
#[tokio::test]
async fn with_the_switch_off_a_sky_is_unsupported_and_no_job_reaches_the_pool() {
    let server = TestServer::start().await;
    let (mut client, universe) = opened(&server).await;
    let before = server.stats();
    let error = refused(&mut client, sky(&universe)).await;
    assert_eq!(error.code, ErrorCode::Unsupported, "{error:?}");
    assert_eq!(error.field, None);
    let after = server.stats();
    assert_eq!(after.pool(), before.pool(), "no job reached the pool");
    assert_eq!(after.sky_tables(), before.sky_tables());
    assert_eq!(after.sky_cells(), before.sky_cells());
    assert_eq!(after.sky_tables().builds(), 0);
    client.close().await;
    server.stop().await;
}

/// The near-Sun test's system to leave out: the first of the Sun's own cell in layer E with a star
/// to draw as a disc.
fn host_record() -> SystemRecord {
    let at = SimPosition::from_light_years(SUN_LY.map(f64::from)).expect("in the cube");
    let mut cell = Vec::new();
    generate_cell(
        galaxy(),
        CellKey::containing(Layer::E, &at).expect("in the cube"),
        &mut cell,
    );
    cell.into_iter()
        .find(|record| {
            let stars = SystemStars::generate(galaxy(), record);
            !host_discs(galaxy(), &stars, hyperion_sim::time::UniverseTime::EPOCH).is_empty()
        })
        .expect("a system of the Sun's cell with a star to draw as a disc")
}

/// `value` × `scale`, rounded to nearest and clamped to [`lo`, `hi`], NaN as 0: the wire's
/// quantisation (Design note 17).
fn quantised(value: f64, scale: f64, lo: f64, hi: f64) -> f64 {
    let scaled = (value * scale).round();
    if scaled.is_nan() {
        0.0
    } else {
        scaled.clamp(lo, hi)
    }
}

/// The wire's chroma of light whose linear Rec. 709 red and green at unit luminance are
/// `red_green`: its chromaticity, as `hyperion_protocol::sky` documents it.
fn chromaticity([r, g]: [f64; 2]) -> [f64; 2] {
    let [yr, yg, yb] = LUMINANCE_RGB;
    let b = (1.0 - yr * r - yg * g) / yb;
    let sum = r + g + b;
    if sum > 0.0 {
        [r / sum, g / sum]
    } else {
        [1.0 / 3.0; 2]
    }
}

/// A fraction of 65,535 as the wire's `u16`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded and clamped to the u16's range first"
)]
fn fraction_bytes(fraction: f64) -> [u8; 2] {
    (quantised(fraction, 65_535.0, 0.0, 65_535.0) as u16).to_le_bytes()
}

/// The bytes the wire carries for the listed star `star`, seen from `observer`, with eye offset
/// `eye_offset`, by `hyperion_protocol::sky`'s table (Design note 17).
#[expect(
    clippy::cast_possible_truncation,
    reason = "the wire's f32 and its integers, each rounded and clamped to its range first"
)]
fn star_wire(observer: &Observer, star: &SkyStar, eye_offset: Magnitudes) -> Vec<u8> {
    let reddened = star.colour().reddened(star.a_v());
    let toward = observer
        .position()
        .displacement_to(star.apparent())
        .metres();
    let direction = UnitVector::from_components(toward).expect("a star away from the observer");
    let mut bytes = Vec::with_capacity(SKY_STAR_BYTES);
    for component in direction.components() {
        bytes.extend_from_slice(&(component as f32).to_le_bytes());
    }
    bytes.extend_from_slice(&(star.distance().value() as f32).to_le_bytes());
    let v = quantised(star.v().value(), 1_000.0, -32_768.0, 32_767.0) as i16;
    bytes.extend_from_slice(&v.to_le_bytes());
    for fraction in chromaticity(reddened.red_green()) {
        bytes.extend_from_slice(&fraction_bytes(fraction));
    }
    bytes.push((quantised(eye_offset.value(), 100.0, -128.0, 127.0) as i8).to_le_bytes()[0]);
    let camera = quantised(reddened.camera_band_mag(), 32.0, -128.0, 127.0) as i8;
    bytes.push(camera.to_le_bytes()[0]);
    bytes
}

/// The bytes the wire carries for the band texel `texel`, by `hyperion_protocol::sky`'s table
/// (Design note 17).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the wire's f32 and its integers, each rounded and clamped to its range first"
)]
fn texel_wire(texel: &BandTexel) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SKY_TEXEL_BYTES);
    bytes.extend_from_slice(&(texel.luminance().value() as f32).to_le_bytes());
    for fraction in chromaticity(texel.chroma().map(f64::from)) {
        bytes.extend_from_slice(&fraction_bytes(fraction));
    }
    let limit = texel.eye_limit().map_or(i16::MIN, |limit| {
        (quantised(limit.value(), 1_000.0, -32_768.0, 32_767.0) as i16).max(i16::MIN + 1)
    });
    bytes.extend_from_slice(&limit.to_le_bytes());
    let ratio = quantised(texel.sp_ratio(), 10_000.0, 0.0, 65_535.0) as u16;
    bytes.extend_from_slice(&ratio.to_le_bytes());
    bytes
}

/// The band of `query`'s census `census`, complete to `complete_to`, at the server's band, its six
/// faces marched on threads of their own, then its limit map and eye offsets: the sim's own, as
/// R06.T9.f's `march_rows` of the server's one reply and `sum_rows`, T9.c's `limit_map` and T9.h's
/// `eye_offsets` give them. It marches the replies the server marches, as T9.f's record asks of
/// this comparison, so that T11.d's several replies change only the replies given.
fn sim_band(
    tables: &Tables,
    query: &SkyQuery,
    census: &SkyCensus,
    complete_to: &CompleteTo,
) -> (Vec<BandTexel>, Vec<Magnitudes>) {
    let spec = BandSpec::STANDARD;
    let faces: Vec<Vec<BandTexel>> = std::thread::scope(|scope| {
        let marching: Vec<_> = CubeFace::ALL
            .chunks(2)
            .map(|faces| {
                scope.spawn(move || {
                    let mut ctx = context_over(tables);
                    faces
                        .iter()
                        .map(|&face| {
                            let march = march_rows(
                                galaxy(),
                                &mut ctx,
                                query,
                                [*complete_to],
                                &spec,
                                face,
                                0..spec.face_texels(),
                            );
                            let mut texels = Vec::new();
                            sum_rows(&march, census, complete_to, &mut texels);
                            texels
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        marching
            .into_iter()
            .flat_map(|faces| faces.join().expect("a face's march"))
            .collect()
    });
    let mut band: Vec<BandTexel> = faces.into_iter().flatten().collect();
    let eye = *query.eye().expect("the eye was asked");
    let eye_cut = query.eye_cut().expect("the eye's cut");
    let glare = Glare::of_listed(query.observer(), census.listed(), &spec, eye_cut);
    limit_map(&eye, &spec, &glare, &mut band);
    let offsets = eye_offsets(&eye, &spec, &glare, &band);
    (band, offsets)
}

/// The host disc `disc` as the reply should carry it.
fn sim_host(disc: &HostDisc) -> HostDiscDto {
    let colour = disc.colour();
    HostDiscDto {
        star: disc.star().get(),
        radius_m: disc.radius().value(),
        teff_k: disc.teff().value(),
        log_g: disc.log_g(),
        mean_luminance_cd_m2: disc.mean_luminance().map(CandelasPerSquareMetre::value),
        central_luminance_cd_m2: disc.central_luminance().map(CandelasPerSquareMetre::value),
        limb: disc.limb().map(|law| hyperion_protocol::PowerTwoDto {
            c: law.c(),
            alpha: law.alpha(),
        }),
        chroma: colour.chroma(),
        lux_per_v0: colour.lux_per_v0(),
        bake_spectrum: colour.bake_spectrum(),
    }
}

/// A sky near the Sun returns the stars, texels and host discs the sim returns for the same query,
/// byte for byte as the wire carries them (R06.T11.c).
///
/// The server builds the galaxy's own tables on its pool, staged from `LuminosityTables::plan`,
/// while the test builds them serially, so the band's light is the tables' and the caps are forced.
/// The request asks the eye and a camera's deeper cut, so the band keeps the eye's light beside its
/// own (R06.T9.j); a small `n_max`, so that the overflow's stars are splatted into the band; and a
/// system of the Sun's cell left out, whose stars come back as discs at the request's time.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sky_near_the_sun_returns_the_stars_texels_and_host_discs_the_sim_returns() {
    // The sim's tables, built serially while the server builds its own on the pool.
    let building = std::thread::spawn(|| tables_of(LuminosityTables::build(galaxy())));
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_galaxy_tables();
    let (server, _data_dir) = sky_server_with(caps, 3).await;
    let (mut client, universe) = opened(&server).await;
    let host = host_record();
    let n_max = 16;
    let sent = served(
        &mut client,
        SkyRequest {
            eye: Some(eye()),
            n_max: Some(n_max),
            exclude_system: Some(SystemIdHex::from_u64(host.id().raw())),
            ..sky(&universe)
        },
    )
    .await;
    let response = &sent.response;
    let tables = building.join().expect("the sim's tables");
    assert_eq!(server.stats().sky_tables().builds(), 1);

    // The same query in the sim: the eye's cut, then the cut, the census, the band and the discs.
    let observer = observer();
    let eye_observer = EyeObserver::default();
    let eye_cut = eye_cut(
        galaxy(),
        &mut context_over(&tables),
        &observer,
        &eye_observer,
        None,
    );
    let cut = eye_cut.value().max(CAMERA_LIMIT_V);
    assert_eq!(response.cut_v.to_bits(), cut.to_bits());
    let query = SkyQuery::builder(observer, Magnitudes::new(cut))
        .eye(eye_observer)
        .eye_cut(eye_cut)
        .n_max(std::num::NonZeroU32::new(n_max).expect("not zero"))
        .exclude(host.id())
        .build()
        .expect("a query")
        .with_caps_forced(LightYears::new(SMALL_CAP_LY))
        .expect("a forced cap");
    let (plan, census) = sim_census_over(&tables, &query);
    assert_eq!(census.listed().len(), usize::try_from(n_max).unwrap());
    assert!(
        !census.overflow().is_empty(),
        "the census overflows into the band"
    );
    assert_eq!(
        (response.listed, response.overflow),
        (
            u32::try_from(census.listed().len()).unwrap(),
            u32::try_from(census.overflow().len()).unwrap()
        )
    );
    let (band, offsets) = sim_band(&tables, &query, &census, &CompleteTo::of_caps(plan.caps()));
    assert!(
        band.iter().all(|texel| texel.luminance().value() > 0.0),
        "the galaxy's light beyond the caps fills every texel"
    );

    // The stars, then the texels, byte for byte.
    let payload = sent.payload();
    assert_eq!(
        (response.stars_bytes, response.band_bytes),
        (u64::from(response.listed) * star_bytes(), band_bytes())
    );
    let (stars, texels) = payload.split_at(usize::try_from(response.stars_bytes).unwrap());
    for (index, ((star, offset), bytes)) in census
        .listed()
        .iter()
        .zip(&offsets)
        .zip(stars.as_chunks::<SKY_STAR_BYTES>().0)
        .enumerate()
    {
        let expected = star_wire(&observer, star, *offset);
        assert_eq!(bytes.as_slice(), expected.as_slice(), "star {index}");
    }
    assert_eq!(band.len(), BAND_TEXELS);
    for (index, (texel, bytes)) in band
        .iter()
        .zip(texels.as_chunks::<SKY_TEXEL_BYTES>().0)
        .enumerate()
    {
        assert!(texel.eye_limit().is_some(), "texel {index} has its limit");
        assert_eq!(
            bytes.as_slice(),
            texel_wire(texel).as_slice(),
            "texel {index}"
        );
    }

    // The host's discs at the request's time.
    let stars = SystemStars::generate(galaxy(), &host);
    let discs = host_discs(galaxy(), &stars, observer.time());
    assert!(!discs.is_empty());
    assert_eq!(
        response.hosts,
        discs.iter().map(sim_host).collect::<Vec<_>>()
    );

    client.close().await;
    server.stop().await;
}

/// Two identical skies asked at once share one build of the galaxy's tables, and send the same
/// bytes (R06.T11.c).
///
/// Their tables hold no star, a build of moments, so the second may find them held rather than
/// join the first's build: `compute::sky_tables`' unit test pins the single flight itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_identical_sky_shares_the_tables_build() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 3).await;
    let (mut first, universe) = opened(&server).await;
    let mut second = server.connected().await;
    let one = first.send_request(RequestBody::Sky(sky(&universe))).await;
    let other = second.send_request(RequestBody::Sky(sky(&universe))).await;
    let (one, other) = (
        answer_to(&mut first, one).await,
        answer_to(&mut second, other).await,
    );
    assert_eq!(one.payload(), other.payload());
    assert_eq!(one.response.bulk, other.response.bulk);
    let tables = server.stats().sky_tables();
    assert_eq!(tables.builds(), 1, "{tables:?}");
    assert_eq!(tables.cache().entries(), 1, "{tables:?}");
    first.close().await;
    second.close().await;
    server.stop().await;
}

/// A sky in another time bucket of DN13's 1,000 years shares the build of the first: one table per
/// galaxy at the reference time serves the whole clock window (decided 2026-10-03,
/// `decision-r06-tables.md`, item B.2; R06.T11.c).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_sky_in_another_time_bucket_shares_the_build() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 3).await;
    let (mut client, universe) = opened(&server).await;
    let now = served(&mut client, sky(&universe)).await.response;
    let built = server.stats().sky_tables();
    assert_eq!(built.builds(), 1, "{built:?}");
    // 900 Julian years before the epoch, within the clock window's ±1,000 years.
    let earlier = UniverseTime {
        seconds: -900 * 31_557_600,
        nanos: 0,
    };
    let then = served(
        &mut client,
        SkyRequest {
            time: earlier,
            ..sky(&universe)
        },
    )
    .await
    .response;
    assert_eq!(then.time, earlier);
    assert_ne!(then.valid_until, now.valid_until);
    let shared = server.stats().sky_tables();
    assert_eq!(shared.builds(), 1, "{shared:?}");
    assert_eq!(
        shared.cache().hits(),
        built.cache().hits() + 1,
        "the second sky found the first's tables: {built:?}, then {shared:?}"
    );
    client.close().await;
    server.stop().await;
}
