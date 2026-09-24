//! The density map over a real socket: its geometry, its codes, its cache, and what else the
//! connection can do while a map is computed (plan 04, P04.T14.c).

mod common;

use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{NETWORK_TIMEOUT, TestClient, TestServer};
use hyperion_protocol::{
    ClientMessage, CreateUniverseRequest, DensityMap, DensityMapRequest, ErrorCode, MapPopulation,
    MapView, OpenUniverseRequest, RequestBody, RequestError, ResponseBody, SeedHex, ServerMessage,
    UniverseIdHex, UniverseInfo,
};
use hyperion_server::compute::{
    CpuPool, DensityMapService, GalaxyCache, GalaxyKey, MapKey, MapResolution, PoolCounters,
};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_sim::{GENERATOR_VERSION, math};
use tokio::time::{sleep, timeout};

/// The seed of every universe these tests create.
const SEED: u64 = 0x4d2;

/// The width in pixels of the maps these tests ask for.
const RESOLUTION: u16 = 128;

/// The width of every map in light-years: the root cube (plan 04, design note 12).
const MAP_WIDTH_LY: f64 = 131_072.0;

async fn connected(server: &TestServer) -> TestClient {
    let mut client = server.connect().await;
    client.hello().await;
    client
}

async fn create(client: &mut TestClient, name: &str) -> UniverseInfo {
    let body = RequestBody::CreateUniverse(CreateUniverseRequest {
        name: name.to_owned(),
        seed: Some(SeedHex::from_u64(SEED)),
    });
    match client.request(body).await {
        Ok(ResponseBody::CreateUniverse(info)) => info,
        other => panic!("expected the created universe, got {other:?}"),
    }
}

/// Opens `universe`, which warms its galaxy on the pool (P04.T14.a), so that a map asked for
/// afterwards finds the worker free for its bands.
async fn open(client: &mut TestClient, universe: &UniverseIdHex) {
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
}

/// Waits until the pool has one job in hand and more queued in bulk, and returns its counters.
///
/// The plan has the integration tests read [`ServerStats`](hyperion_server::ServerStats) rather than
/// guess at timing (P04.T13.c); the pool's counters are a snapshot and not a watch, so this polls
/// them, bounded by [`NETWORK_TIMEOUT`] so that a map that never reaches the pool fails the test
/// instead of hanging the suite.
async fn bulk_work_under_way(server: &TestServer) -> PoolCounters {
    timeout(NETWORK_TIMEOUT, async {
        loop {
            let pool = server.stats().pool();
            if pool.running() > 0 && pool.queued_bulk() > 0 {
                return pool;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the map's bands never reached the pool")
}

fn request(
    universe: &UniverseIdHex,
    view: MapView,
    population: MapPopulation,
    resolution: u16,
    bits: u8,
) -> RequestBody {
    RequestBody::DensityMap(DensityMapRequest {
        universe: universe.clone(),
        view,
        population,
        resolution,
        bits,
    })
}

async fn map(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    view: MapView,
    population: MapPopulation,
    bits: u8,
) -> Result<DensityMap, RequestError> {
    let body = request(universe, view, population, RESOLUTION, bits);
    client.request(body).await.map(|response| match response {
        ResponseBody::DensityMap(map) => map,
        other => panic!("expected a density map, got {other:?}"),
    })
}

/// The codes of a map, decoded as the client's decoder decodes them, with the byte count checked
/// against the header.
fn codes(map: &DensityMap) -> Vec<u16> {
    let bytes = STANDARD
        .decode(&map.data_base64)
        .expect("the server sends standard base64");
    let pixels = usize::from(map.width_px) * usize::from(map.height_px);
    assert_eq!(
        bytes.len(),
        pixels * usize::from(map.bits) / 8,
        "the byte count is the header's pixels times its bits"
    );
    match map.bits {
        8 => bytes.into_iter().map(u16::from).collect(),
        16 => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect(),
        other => panic!("unexpected bit depth {other}"),
    }
}

/// The index of the pixel in column `column` and row `row`.
fn at(map: &DensityMap, column: u16, row: u16) -> usize {
    usize::from(row) * usize::from(map.width_px) + usize::from(column)
}

#[tokio::test]
async fn both_views_and_both_depths_carry_their_own_codes_and_geometry() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;

    for view in [MapView::FaceOn, MapView::EdgeOn] {
        for bits in [8_u8, 16] {
            let map = map(&mut client, &universe.id, view, MapPopulation::All, bits)
                .await
                .unwrap();
            assert_eq!(map.universe, universe.id);
            assert_eq!(map.view, view);
            assert_eq!(map.population, MapPopulation::All);
            assert_eq!(map.bits, bits);
            assert_eq!(map.width_px, RESOLUTION);
            let expected_height = match view {
                MapView::FaceOn => RESOLUTION,
                MapView::EdgeOn => RESOLUTION / 2,
            };
            assert_eq!(map.height_px, expected_height, "{view:?}");
            assert_eq!(map.centre_ly.map(f64::to_bits), [0.0_f64.to_bits(); 2]);
            // The picture spans the root cube: a pixel times the width is 131,072 ly.
            assert!(
                (map.ly_per_px * f64::from(map.width_px) - MAP_WIDTH_LY).abs() < 1e-9,
                "{} ly per px over {} px",
                map.ly_per_px,
                map.width_px
            );
            // The codes decode to one per pixel, and the map's ceiling gets the largest code.
            let codes = codes(&map);
            assert_eq!(
                codes.len(),
                usize::from(map.width_px) * usize::from(map.height_px)
            );
            let largest = codes.iter().copied().max().expect("the map has pixels");
            let max_code = if bits == 8 { 255 } else { 65_535 };
            assert_eq!(largest, max_code, "{view:?} at {bits} bits");
            assert!(
                map.floor_log10_per_ly2 < map.ceiling_log10_per_ly2,
                "{map:?}"
            );
        }
    }
    // Two views at two depths, and one raster per view: the depth is quantised from the raster.
    assert_eq!(server.stats().maps().entries(), 2);

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn the_centre_pixel_of_a_face_on_map_holds_the_ceiling() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;
    let map = map(
        &mut client,
        &universe.id,
        MapView::FaceOn,
        MapPopulation::All,
        8,
    )
    .await
    .unwrap();
    let codes = codes(&map);
    let centre = at(&map, map.width_px / 2, map.height_px / 2);
    assert_eq!(
        codes[centre], 255,
        "the galactic centre is the densest column"
    );
    // The corner of the picture looks 65,000 ly out of the plane and past the halo's cut.
    assert_eq!(codes[at(&map, 0, 0)], 0);

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn the_young_disc_and_every_population_give_different_maps() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;
    let all = map(
        &mut client,
        &universe.id,
        MapView::FaceOn,
        MapPopulation::All,
        8,
    )
    .await
    .unwrap();
    let young = map(
        &mut client,
        &universe.id,
        MapView::FaceOn,
        MapPopulation::Young,
        8,
    )
    .await
    .unwrap();
    assert_ne!(
        codes(&all),
        codes(&young),
        "the young disc is where the arms show, and its map is its own"
    );
    assert_eq!(server.stats().maps().entries(), 2);

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_repeated_request_is_served_from_the_cache() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;
    let first = map(
        &mut client,
        &universe.id,
        MapView::FaceOn,
        MapPopulation::All,
        8,
    )
    .await
    .unwrap();
    assert_eq!(server.stats().maps().hits(), 0);

    let again = map(
        &mut client,
        &universe.id,
        MapView::FaceOn,
        MapPopulation::All,
        8,
    )
    .await
    .unwrap();
    assert_eq!(again, first, "the cached map answers identically");
    // The other depth of the same map is the same raster, quantised again.
    let deeper = map(
        &mut client,
        &universe.id,
        MapView::FaceOn,
        MapPopulation::All,
        16,
    )
    .await
    .unwrap();
    assert_eq!(deeper.bits, 16);
    assert_eq!(
        deeper.ceiling_log10_per_ly2.to_bits(),
        first.ceiling_log10_per_ly2.to_bits(),
        "the same raster gives the same ceiling at either depth"
    );
    let maps = server.stats().maps();
    assert_eq!((maps.hits(), maps.entries()), (2, 1));

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_bad_resolution_or_bit_depth_is_a_bad_request_naming_the_field() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;

    for resolution in [0_u16, 64, 127, 129, 2_048] {
        let body = request(
            &universe.id,
            MapView::FaceOn,
            MapPopulation::All,
            resolution,
            8,
        );
        let error = client.request(body).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::BadRequest, "{resolution}");
        assert_eq!(error.field.as_deref(), Some("resolution"), "{resolution}");
        assert!(error.message.contains("128, 256, 512 or 1024"), "{error:?}");
    }
    for bits in [0_u8, 1, 4, 12, 32] {
        let body = request(
            &universe.id,
            MapView::FaceOn,
            MapPopulation::All,
            RESOLUTION,
            bits,
        );
        let error = client.request(body).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::BadRequest, "{bits}");
        assert_eq!(error.field.as_deref(), Some("bits"), "{bits}");
    }
    // A request that never named a map computed nothing and looked nothing up.
    let maps = server.stats().maps();
    assert_eq!((maps.entries(), maps.misses()), (0, 0));

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn an_unknown_universe_is_refused_before_any_map_is_computed() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let error = map(
        &mut client,
        &UniverseIdHex::from_u64(0xdead),
        MapView::FaceOn,
        MapPopulation::All,
        8,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownUniverse);
    assert_eq!(error.field.as_deref(), Some("universe"));
    assert_eq!(server.stats().maps().entries(), 0);

    client.close().await;
    server.stop().await;
}

/// The socket does not stall while a map's bands are computed, and the map can be given up.
///
/// One worker and the dearest raster there is: a 1,024-pixel edge-on map is 32 bands of some
/// seconds each on one worker (see the plan's Risks for the measured cost), so nothing here waits
/// for it. The galaxy is warmed by `open_universe` first, or the one worker would still be building
/// it when the `cancel` arrives and no band would ever be queued; the pool's counters are then read
/// before the `ping`, so that the test fails rather than quietly passes if the map never reached the
/// pool at all. `ping` is answered by the connection task on the runtime, never by the pool, and
/// `cancel` ends the request at once while the band in hand runs on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_map_in_flight_blocks_neither_ping_nor_cancel() {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(NonZeroUsize::new(1).expect("one worker"))
        .build();
    let server = TestServer::start_with(config).await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;
    open(&mut client, &universe.id).await;
    assert_eq!(server.stats().galaxies().builds(), 1, "the galaxy is warm");

    let map_id = client
        .send_request(request(
            &universe.id,
            MapView::EdgeOn,
            MapPopulation::All,
            1_024,
            8,
        ))
        .await;
    // The map is the pool's bulk work and the only worker is inside one of its bands, which is what
    // the `ping` below has to overtake.
    let pool = bulk_work_under_way(&server).await;
    assert_eq!(
        pool.running(),
        1,
        "the one worker has a band in hand: {pool:?}"
    );
    // A 512-row raster is 32 bands of 16 rows, so 31 wait behind the one in hand. Asserted loosely
    // because the band size is a knob the plan reserves, and because a band is seconds of work that
    // a heavily loaded machine could see finish before this reads the counters.
    assert!(pool.queued_bulk() > 0, "bands are queued: {pool:?}");

    // The pong arrives while a band of the map is being computed.
    client.send(&ClientMessage::Ping { nonce: 7 }).await;
    match client.next_message().await {
        ServerMessage::Pong { nonce } => assert_eq!(nonce, 7),
        other => panic!("expected the pong before the map, got {other:?}"),
    }

    client.cancel(map_id).await;
    match client.next_message().await {
        ServerMessage::RequestError { id, error } => {
            assert_eq!(id, map_id);
            assert_eq!(error.code, ErrorCode::Cancelled);
        }
        other => panic!("expected the map's `cancelled`, got {other:?}"),
    }
    // The connection is healthy after giving up a map, and the map never answered.
    match client.ping(8).await {
        ServerMessage::Pong { nonce } => assert_eq!(nonce, 8),
        other => panic!("expected a pong, got {other:?}"),
    }
    // Three requests were made: the create and the open, which answered, and the map, which was
    // cancelled. A `ping` is not a request and is answered by the connection itself.
    let requests = server.stats().requests();
    assert_eq!((requests.cancelled(), requests.responded()), (1, 2));
    assert_eq!(
        server.stats().maps().entries(),
        0,
        "the map was given up, so nothing was cached"
    );

    client.close().await;
    // The bands still queued were skipped when the last waiter went; the band in hand runs to its
    // end, which is what `SHUTDOWN_TIMEOUT` allows for.
    server.stop().await;
}

/// The work in both 512-pixel rasters of the fixture galaxy, in calls of the sim's `math::exp`.
///
/// Measured band by band on one thread, each band against a calibration of the call taken either
/// side of it, so that the count does not move with the host's clock or load (plan 04, Risks, the
/// validation of T16): 5.2 to 5.6 × 10⁹ at generator version 8, of which 4.1 to 4.6 × 10⁸ face-on
/// and 4.8 to 5.2 × 10⁹ edge-on. At the 7.5 ns a call that an idle i7-8665U gives at its 4.2 GHz
/// single-core clock, that is about 40 core-seconds. The upper figures are used.
const RASTER_PAIR_EXP_CALLS: f64 = 5.6e9;

/// The face-on raster's share of [`RASTER_PAIR_EXP_CALLS`].
const FACE_ON_EXP_CALLS: f64 = 4.6e8;

/// The dearest single band of the pair, in the same calls: an edge-on band of 16 rows through the
/// plane, 4.4 to 5.5 × 10⁸. A map is done when its dearest band is, however many workers there are,
/// so this bounds the build from below on a host with more workers than the edge-on map has bands.
const DEAREST_BAND_EXP_CALLS: f64 = 5.5e8;

/// How many times the expected work a build may take before the test fails.
///
/// The expected work is [`RASTER_PAIR_EXP_CALLS`] ÷ workers, or the face-on map's share of it plus
/// [`DEAREST_BAND_EXP_CALLS`] where that is longer. Measured in the slow-test profile against the
/// calibration below, builds came to 0.27 to 0.95 of it, on eight workers and on two at load
/// averages of 9 to 17: less than one because a hyperthread slows a loop of pure `exp` more than it
/// slows the raster, and spread by load that moved between a calibration and the build. Three times
/// is wide enough for a loaded gate and for a generator bump that moves plan 02's raster cost by a
/// half. What it catches depends on the host: on this laptop's eight hyperthreads a build runs at
/// about 0.57 of the expected work, so it fails at a per-pixel regression of about five times, and
/// four times, tried, passed. It is a guard against gross regressions, not a benchmark.
const BUILD_SLACK: f64 = 3.0;

/// `math::exp` calls on each calibrating thread in one pass: about 30 ms of an idle core, several
/// of the scheduler's time slices, so that a thread's share of a loaded host averages out.
const CALIBRATION_CALLS: u32 = 4_000_000;

/// Passes per calibration, whose median is taken.
const CALIBRATION_PASSES: usize = 5;

/// How often the build's pool is looked at, to see how many bands it is running at once.
const RUNNING_POLL: Duration = Duration::from_millis(2);

/// Nanoseconds a call of the sim's `math::exp` takes on each of `threads` threads running at once:
/// every thread times its own calls, the pass is their mean, and the calibration is the median of
/// [`CALIBRATION_PASSES`] passes.
///
/// This is the host's speed as each of the pool's workers sees it, measured in this process and at
/// the pool's width: the clock, which on this laptop moves between 1.9 and 4.8 GHz with load and
/// heat, and the contention, from other processes and from hyperthreads sharing a core, that the
/// workers meet. Each thread is timed on its own, not the pass as a whole, because a pass that
/// waits for its slowest thread measures how late the scheduler ran one thread rather than how fast
/// the threads ran; the first read eight threads on this laptop at 110–130 ns a call under a load of
/// 20, against a build that ran as though at 25.
fn exp_ns_at(threads: NonZeroUsize) -> f64 {
    let mut passes: Vec<f64> = (0..CALIBRATION_PASSES)
        .map(|_| {
            let per_thread: Vec<Duration> = thread::scope(|scope| {
                let timers: Vec<_> = (0..threads.get())
                    .map(|_| {
                        scope.spawn(|| {
                            let started = Instant::now();
                            let mut sum = 0.0_f64;
                            for call in 0..CALIBRATION_CALLS {
                                sum += math::exp(black_box(f64::from(call % 97) * 0.01));
                            }
                            black_box(sum);
                            started.elapsed()
                        })
                    })
                    .collect();
                timers
                    .into_iter()
                    .map(|timer| timer.join().expect("a calibrating thread does not panic"))
                    .collect()
            });
            let total: Duration = per_thread.iter().sum();
            let calls = f64::from(CALIBRATION_CALLS)
                * f64::from(u32::try_from(threads.get()).expect("fewer than 2³² threads"));
            total.as_secs_f64() * 1e9 / calls
        })
        .collect();
    passes.sort_unstable_by(f64::total_cmp);
    passes[CALIBRATION_PASSES / 2]
}

/// Both 512-pixel maps of one galaxy build with every worker running a band at once, and inside a
/// budget stated in the host's own `math::exp` calls (P04.T16).
///
/// The build is what is timed, through the cache and the pool a server uses, not a response: the
/// quantising and the base64 of one are milliseconds (`benches/density_map.rs`) against seconds of
/// raster. The galaxy is built by the first `get`, as it is for the first request of a new universe,
/// and is counted in.
///
/// Two things are checked, because a wall-clock budget alone checks neither. That the bands run in
/// parallel is read off the pool, which must be seen running as many bands at once as it has workers
/// (or bands, if fewer): serial bands cost only two to five times the parallel build, which no budget
/// wide enough for a loaded machine can tell apart. And the cost: the build must finish within
/// [`BUILD_SLACK`] times its expected work, counted in calls of `math::exp` as timed at the pool's
/// width before and after it, the slower of the two, so that the budget follows the host's clock
/// and load instead of assuming one.
///
/// The plan asked for 20 s of wall clock on CI's two cores. That is just out of reach, not far out
/// of it: the pair is about 40 core-seconds at this laptop's best single-core clock, so two cores
/// need at least 20 s there and more on a slower host. The 180 s that this test held before was
/// set from measurements taken under a load of 8 to 29 and never normalised, which made the pair
/// look like 61 core-seconds, and was wide enough to pass bands run one at a time.
#[tokio::test]
#[ignore = "slow: both 512-pixel rasters, about 40 core-seconds of arithmetic"]
async fn density_map_512_builds_within_budget() {
    let workers = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let pool = Arc::new(
        CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
            .expect("the pool starts"),
    );
    let galaxies = Arc::new(GalaxyCache::new(Arc::clone(&pool)));
    // Room for both maps: 512 × 512 and 512 × 256 pixels of `f32`, 768 KiB in all.
    let maps = DensityMapService::new(Arc::clone(&pool), Arc::clone(&galaxies), 4 << 20);

    let exp_before = exp_ns_at(workers);
    let started = Instant::now();
    let build = async {
        for view in [MapView::FaceOn, MapView::EdgeOn] {
            let key = MapKey::new(
                GalaxyKey::new(SEED, GENERATOR_VERSION),
                view,
                MapPopulation::All,
                MapResolution::Px512,
            );
            let map = maps.get(key).await.expect("the pool computes the map");
            assert_eq!(
                map.log10().len(),
                usize::from(MapResolution::Px512.width_px())
                    * usize::from(MapResolution::Px512.height_px(view)),
                "the {view:?} map holds one value per pixel"
            );
        }
    };
    tokio::pin!(build);
    let mut most_running = 0;
    loop {
        tokio::select! {
            () = &mut build => break,
            () = sleep(RUNNING_POLL) => most_running = most_running.max(pool.counters().running()),
        }
    }
    let elapsed = started.elapsed();
    let exp_after = exp_ns_at(workers);
    pool.shutdown().await.expect("the pool shuts down");

    // The edge-on map has the fewer bands: 256 rows of 16.
    let bands = 16;
    assert_eq!(
        most_running,
        workers.get().min(bands),
        "the pool of {workers} workers ran at most {most_running} bands at once"
    );
    let exp_ns = exp_before.max(exp_after);
    let workers_f64 = f64::from(u32::try_from(workers.get()).expect("fewer than 2³² workers"));
    let budget_calls = BUILD_SLACK
        * (RASTER_PAIR_EXP_CALLS / workers_f64)
            .max(FACE_ON_EXP_CALLS / workers_f64 + DEAREST_BAND_EXP_CALLS);
    let budget = Duration::from_secs_f64(budget_calls * exp_ns * 1e-9);
    eprintln!(
        "both 512-pixel maps took {elapsed:?} on {workers} workers against a budget of {budget:?}: \
         {:.2} of the expected work",
        elapsed.as_secs_f64() * 1e9 / exp_ns / (RASTER_PAIR_EXP_CALLS / workers_f64)
    );
    assert!(
        elapsed <= budget,
        "both 512-pixel maps took {elapsed:?} on {workers} workers, over the budget of {budget:?}: \
         {budget_calls:.3e} calls of `math::exp` at {exp_ns:.2} ns (before {exp_before:.2}, after \
         {exp_after:.2}); the build was {:.3e} calls, {:.2} of the expected work",
        elapsed.as_secs_f64() * 1e9 / exp_ns,
        elapsed.as_secs_f64() * 1e9 / exp_ns / (RASTER_PAIR_EXP_CALLS / workers_f64)
    );
}
