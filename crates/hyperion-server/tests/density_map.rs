//! The density map over a real socket: its geometry, its codes, its cache, and what else the
//! connection can do while a map is computed (plan 04, P04.T14.c).

mod common;

use std::num::NonZeroUsize;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{TestClient, TestServer};
use hyperion_protocol::{
    ClientMessage, CreateUniverseRequest, DensityMap, DensityMapRequest, ErrorCode, MapPopulation,
    MapView, RequestBody, RequestError, ResponseBody, SeedHex, ServerMessage, UniverseIdHex,
    UniverseInfo,
};

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

/// The socket does not stall while a map is computed, and the map can be given up.
///
/// One worker and the dearest raster there is: a 1,024-pixel edge-on map is minutes of work on one
/// worker (see the plan's Risks for the measured cost), so nothing here waits for it. `ping` is
/// answered by the connection task on the runtime, never by the pool, and `cancel` ends the request
/// at once while the band in hand runs on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_map_in_flight_blocks_neither_ping_nor_cancel() {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(NonZeroUsize::new(1).expect("one worker"))
        .build();
    let server = TestServer::start_with(config).await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach").await;

    let map_id = client
        .send_request(request(
            &universe.id,
            MapView::EdgeOn,
            MapPopulation::All,
            1_024,
            8,
        ))
        .await;
    // The pong arrives while the map is still being computed.
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
    // Two requests were made: the create, which answered, and the map, which was cancelled. A
    // `ping` is not a request and is answered by the connection itself.
    let requests = server.stats().requests();
    assert_eq!((requests.cancelled(), requests.responded()), (1, 1));
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
