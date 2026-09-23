//! Galaxy creation from end to end, over one socket, as the `GALAXY` display will walk it (plan 04,
//! P04.T14.e).
//!
//! `hello`, `create_universe`, `galaxy_parameters`, a face-on and an edge-on map, a chart centre
//! taken from those maps by the rule their own doc comment states, and the systems around that
//! point at two times. Then the server is stopped, another is started on the same data directory,
//! and the same query is asked again: its answer must be byte for byte the first one's. This is the
//! test that fails if any seam between plans 01–03 and the wire moves.

mod common;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{TestClient, TestServer};
use hyperion_protocol::{
    DensityMap, DensityMapRequest, GalacticPosition, GalaxyParameters, GalaxyParametersRequest,
    MapPopulation, MapView, MassLayer, OpenUniverseRequest, RequestBody, ResponseBody, SeedHex,
    ServerMessage, SystemsInRange, SystemsInRangeRequest, UniverseIdHex, UniverseList,
    UniverseTime,
};
use hyperion_server::limits::MAX_CENSUS_LIMIT;

/// The seed the plan's walk-through uses: 1,234.
const SEED: u64 = 0x4d2;

/// The width in pixels of both maps.
const RESOLUTION: u16 = 128;

/// The radius of the chart's queries, light-years.
const RADIUS_LY: f64 = 50.0;

/// About where the chart is centred: the solar circle, 26,000 ly out from the centre.
const CHART_RADIUS_LY: f64 = 26_000.0;

/// Seconds in a Julian year, the sim's year.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

async fn parameters(client: &mut TestClient, universe: &UniverseIdHex) -> GalaxyParameters {
    let body = RequestBody::GalaxyParameters(GalaxyParametersRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::GalaxyParameters(parameters)) => parameters,
        other => panic!("expected the galaxy's parameters, got {other:?}"),
    }
}

async fn map(client: &mut TestClient, universe: &UniverseIdHex, view: MapView) -> DensityMap {
    let body = RequestBody::DensityMap(DensityMapRequest {
        universe: universe.clone(),
        view,
        population: MapPopulation::All,
        resolution: RESOLUTION,
        bits: 8,
    });
    match client.request(body).await {
        Ok(ResponseBody::DensityMap(map)) => map,
        other => panic!("expected a density map, got {other:?}"),
    }
}

/// The request for the systems within [`RADIUS_LY`] of `centre` at `time`.
fn query(universe: &UniverseIdHex, centre: GalacticPosition, time: UniverseTime) -> RequestBody {
    RequestBody::SystemsInRange(SystemsInRangeRequest {
        universe: universe.clone(),
        centre,
        radius_ly: RADIUS_LY,
        time,
        min_layer: MassLayer::A,
        limit: MAX_CENSUS_LIMIT,
        include_stellar: false,
    })
}

/// Asks `body` and returns the answer with the text of the frame it arrived in.
async fn ask(client: &mut TestClient, body: RequestBody) -> (SystemsInRange, String) {
    let id = client.send_request(body).await;
    let text = client.next_text().await;
    let message: ServerMessage =
        serde_json::from_str(&text).expect("the server sends valid messages");
    match message {
        ServerMessage::Response {
            id: answered,
            body: ResponseBody::SystemsInRange(answer),
        } if answered == id => (answer, text),
        other => panic!("expected the systems in range, got {other:?}"),
    }
}

/// The part of a response frame that is the answer itself, without the request's ID: everything
/// from `"body":` on, which two answers share exactly when their bodies do.
fn answer_bytes(frame: &str) -> &str {
    let body = frame
        .find("\"body\":")
        .expect("a response frame carries a body");
    &frame[body..]
}

/// The light-years at the centre of the pixel in column `column` and row `row`, by the rule the
/// [`DensityMap`] doc comment states: `(x, y)` face-on and `(x, z)` edge-on.
fn pixel_centre_ly(map: &DensityMap, column: u16, row: u16) -> (f64, f64) {
    let half = |pixels: u16| f64::from(pixels) / 2.0;
    (
        map.centre_ly[0] + (f64::from(column) + 0.5 - half(map.width_px)) * map.ly_per_px,
        map.centre_ly[1] + (half(map.height_px) - f64::from(row) - 0.5) * map.ly_per_px,
    )
}

/// The pixel of `map` whose centre is nearest `(horizontal_ly, vertical_ly)`, as picking a point on
/// the display picks a pixel. At 128 pixels one of them is 1,024 ly across, so this is as finely as
/// the map can name a place (plan 04, Risks).
fn pixel_nearest(map: &DensityMap, horizontal_ly: f64, vertical_ly: f64) -> (u16, u16) {
    // Each index is the doc comment's rule read backwards; the vertical axis counts downwards, so
    // its offset from the raster's centre is subtracted rather than added.
    let columns = f64::from(map.width_px) / 2.0 - 0.5;
    let rows = f64::from(map.height_px) / 2.0 - 0.5;
    (
        pixel_index(
            columns + (horizontal_ly - map.centre_ly[0]) / map.ly_per_px,
            map.width_px,
        ),
        pixel_index(
            rows - (vertical_ly - map.centre_ly[1]) / map.ly_per_px,
            map.height_px,
        ),
    )
}

/// The nearest whole pixel index to `index`, inside the raster.
fn pixel_index(index: f64, pixels: u16) -> u16 {
    let inside = index.round().clamp(0.0, f64::from(pixels - 1));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the clamp holds the value in 0..=pixels − 1, which a `u16` holds, and `round` \
                  has already dropped the fraction"
    )]
    let pixel = inside as u16;
    pixel
}

/// A position in the galactic frame from light-years, by the rule the client's
/// `galacticPositionFromLy` follows: the whole light-years are the cell and the rest is the
/// offset in metres. The sim's own conversion is that rule, so the test states it once, not twice.
fn position_from_ly(xyz_ly: [f64; 3]) -> GalacticPosition {
    let position = hyperion_sim::coords::GalacticPosition::from_light_years(xyz_ly)
        .expect("a chart centre taken from the map lies inside the root cube");
    GalacticPosition {
        cell_ly: position.cell().to_array(),
        offset_m: position.offset_metres(),
    }
}

/// The codes of a map, one per pixel, with the byte count checked against the header.
fn codes(map: &DensityMap) -> Vec<u8> {
    let bytes = STANDARD
        .decode(&map.data_base64)
        .expect("the server sends standard base64");
    assert_eq!(
        bytes.len(),
        usize::from(map.width_px) * usize::from(map.height_px),
        "an 8-bit map is one byte per pixel"
    );
    bytes
}

/// A chart centre picked off the maps, as the display picks one: x and y from the centre of a
/// face-on pixel about 26,000 ly out from the galactic centre, and z from the edge-on row nearest the
/// plane. At 128 pixels no row is centred on the plane, so that row's centre is half a pixel, 512 ly,
/// from it.
fn chart_centre(face_on: &DensityMap, edge_on: &DensityMap) -> GalacticPosition {
    let (column, row) = pixel_nearest(face_on, 0.0, CHART_RADIUS_LY);
    let (x_ly, y_ly) = pixel_centre_ly(face_on, column, row);
    let (edge_column, edge_row) = pixel_nearest(edge_on, x_ly, 0.0);
    let (edge_x_ly, z_ly) = pixel_centre_ly(edge_on, edge_column, edge_row);
    assert!(
        (edge_x_ly - x_ly).abs() <= f64::EPSILON * x_ly.abs(),
        "both maps are the same width, so the same x is the same column: {x_ly} and {edge_x_ly}"
    );
    assert!(
        (y_ly - CHART_RADIUS_LY).abs() <= face_on.ly_per_px,
        "{y_ly} ly is not within a pixel of the solar circle"
    );
    assert!(
        z_ly.abs() <= edge_on.ly_per_px,
        "{z_ly} ly is not within a pixel of the plane"
    );
    position_from_ly([x_ly, y_ly, z_ly])
}

#[tokio::test]
async fn a_universe_is_created_charted_and_reopened_over_one_socket() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;

    // Creation: a name and the seed the operator typed.
    let universe = client.create_universe("Kepler Reach", SEED).await;
    assert_eq!(universe.seed, SeedHex::from_u64(SEED));
    assert_eq!(universe.seed.as_str(), "00000000000004d2");

    // The drawn parameters: the seed echoes, and the masses are there for the display's table.
    let parameters = parameters(&mut client, &universe.id).await;
    assert_eq!(parameters.universe, universe.id);
    assert_eq!(parameters.seed, universe.seed);
    let mass = parameters
        .groups
        .iter()
        .find(|group| group.key == "mass")
        .expect("the `mass` group is sent");
    assert!(
        mass.parameters
            .iter()
            .any(|parameter| parameter.key == "stellar_mass"),
        "{mass:?}"
    );

    // The two maps the display draws.
    let face_on = map(&mut client, &universe.id, MapView::FaceOn).await;
    let edge_on = map(&mut client, &universe.id, MapView::EdgeOn).await;
    assert_eq!(
        (face_on.width_px, face_on.height_px),
        (RESOLUTION, RESOLUTION)
    );
    assert_eq!(
        (edge_on.width_px, edge_on.height_px),
        (RESOLUTION, RESOLUTION / 2)
    );
    let drawn = |map: &DensityMap| codes(map).into_iter().filter(|&code| code > 0).count();
    assert!(
        drawn(&face_on) > 0 && drawn(&edge_on) > 0,
        "both maps have systems in them"
    );

    let centre = chart_centre(&face_on, &edge_on);

    // The systems around that point, at the epoch and a century later.
    let epoch = UniverseTime {
        seconds: 0,
        nanos: 0,
    };
    let century = UniverseTime {
        seconds: 100 * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    };
    let (at_epoch, at_epoch_bytes) = ask(&mut client, query(&universe.id, centre, epoch)).await;
    assert_eq!(at_epoch.time, epoch);
    assert_eq!(at_epoch.centre, centre);
    assert!(
        !at_epoch.systems.is_empty(),
        "fifty light-years of the solar circle holds systems"
    );
    let (later, _) = ask(&mut client, query(&universe.id, centre, century)).await;
    assert_eq!(later.time, century);
    assert!(!later.systems.is_empty());

    // Nothing but the identity is saved, so another server on the same directory answers the same
    // query with the same bytes.
    client.close().await;
    let server = server.restart().await;
    let mut client = server.connected().await;
    let listed = match client.request(RequestBody::ListUniverses).await {
        Ok(ResponseBody::ListUniverses(list)) => list,
        other => panic!("expected the universe list, got {other:?}"),
    };
    assert_eq!(
        listed,
        UniverseList {
            universes: vec![universe.clone()],
            server_generator_version: universe.generator_version,
        }
    );
    let opened = client
        .request(RequestBody::OpenUniverse(OpenUniverseRequest {
            universe: universe.id.clone(),
        }))
        .await;
    assert_eq!(opened, Ok(ResponseBody::OpenUniverse(universe.clone())));
    // The new server rebuilt the galaxy from the save's seed and holds no cell yet, so the answer
    // below is computed afresh from the identity on disk and is not the first server's, cached.
    let stats = server.stats();
    assert_eq!(
        (stats.galaxies().builds(), stats.cells().entries()),
        (1, 0),
        "the restarted server starts from the save alone"
    );
    let (again, again_bytes) = ask(&mut client, query(&universe.id, centre, epoch)).await;
    let cells = server.stats().cells();
    assert!(
        cells.entries() > 0 && cells.hits() == 0,
        "the re-answer generated its cells: {cells:?}"
    );
    assert_eq!(again, at_epoch);
    assert_eq!(
        answer_bytes(&again_bytes),
        answer_bytes(&at_epoch_bytes),
        "the same query of the same seed answers with the same bytes after a restart"
    );

    client.close().await;
    server.stop().await;
}
