//! The galaxy parameters over a real socket: the response for a fixed seed, pinned, and what the
//! galaxy cache does behind it (plan 04, P04.T14.b).

mod common;

use common::{TestClient, TestServer, pretty_json_frame};
use hyperion_protocol::{
    CreateUniverseRequest, GalaxyParameters, GalaxyParametersRequest, OpenUniverseRequest,
    RequestBody, RequestError, ResponseBody, SeedHex, ServerMessage, UniverseIdHex, UniverseInfo,
};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed the plan's end-to-end walk uses, `00000000000004d2`.
const SEED: u64 = 0x4d2;

async fn connected(server: &TestServer) -> TestClient {
    let mut client = server.connect().await;
    client.hello().await;
    client
}

async fn create(client: &mut TestClient, name: &str, seed: u64) -> UniverseInfo {
    let body = RequestBody::CreateUniverse(CreateUniverseRequest {
        name: name.to_owned(),
        seed: Some(SeedHex::from_u64(seed)),
    });
    match client.request(body).await {
        Ok(ResponseBody::CreateUniverse(info)) => info,
        other => panic!("expected the created universe, got {other:?}"),
    }
}

async fn parameters(
    client: &mut TestClient,
    universe: &UniverseIdHex,
) -> Result<GalaxyParameters, RequestError> {
    let body = RequestBody::GalaxyParameters(GalaxyParametersRequest {
        universe: universe.clone(),
    });
    client.request(body).await.map(|response| match response {
        ResponseBody::GalaxyParameters(parameters) => parameters,
        other => panic!("expected the galaxy parameters, got {other:?}"),
    })
}

async fn open(client: &mut TestClient, universe: &UniverseIdHex) -> UniverseInfo {
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(info)) => info,
        other => panic!("expected the opened universe, got {other:?}"),
    }
}

/// The parameters of a fixed seed on one server: the golden response, and two universes of the seed
/// sharing their galaxy and its parameters.
///
/// Nextest runs each test in its own process, and each server builds its galaxy, 4.4 s with its
/// full potential in a test build, so the two share one server: each takes a client and its
/// universes of its own, as it did with a server of its own, and its assertions and their messages
/// are unchanged. The golden pins the server's first universe and its client's requests, so it
/// comes first. The two universes then find the galaxy the golden's request built: their one build
/// and one entry are still the cache's keying by seed and version, since a galaxy built for each
/// universe would make three. The tests that count from a fresh server
/// (`open_warms_the_galaxy_so_the_parameters_need_no_build`,
/// `an_unknown_universe_is_refused_before_any_galaxy_is_built`) keep a server each. A server holds
/// one universe of a name, so each property's universe is named apart (no assertion reads a name),
/// the first keeping its test's.
#[tokio::test]
async fn the_golden_parameters_are_shared_by_every_universe_of_the_seed() {
    let server = TestServer::start().await;
    the_parameters_of_a_fixed_seed_are_the_golden_response(&server).await;
    two_universes_of_one_seed_share_their_galaxy_and_its_parameters(&server).await;
    server.stop().await;
}

/// Run first by [`the_golden_parameters_are_shared_by_every_universe_of_the_seed`] on its shared
/// server, so that its universe is the server's first, as the golden's is.
async fn the_parameters_of_a_fixed_seed_are_the_golden_response(server: &TestServer) {
    let mut client = connected(server).await;
    let universe = create(&mut client, "Kepler Reach", SEED).await;
    let response = parameters(&mut client, &universe.id).await.unwrap();
    assert_eq!(response.seed, SeedHex::from_u64(SEED));
    assert_eq!(response.generator_version, GENERATOR_VERSION.get());

    // The frame as it arrived, under the golden header that ties it to the generator version (a
    // version bump re-blesses this file), laid out as pretty JSON with every number copied byte for
    // byte (`common::pretty_json_frame`). It is not written from the parsed response: `serde_json`
    // without `float_roundtrip` reads a few of these floats a last bit out (nine of them, when the
    // golden was first written from the parse), so a golden of the parse could not see the server
    // move one of them by an ulp.
    let id = client
        .send_request(RequestBody::GalaxyParameters(GalaxyParametersRequest {
            universe: universe.id.clone(),
        }))
        .await;
    let frame = client.next_text().await;
    match serde_json::from_str(&frame).unwrap() {
        ServerMessage::Response {
            id: answered,
            body: ResponseBody::GalaxyParameters(again),
        } if answered == id => assert_eq!(again, response),
        other => panic!("expected the galaxy parameters, got {other:?}"),
    }
    let mut golden = GoldenWriter::new();
    golden.header(GENERATOR_VERSION.get());
    for line in pretty_json_frame(&frame).lines() {
        golden.line(line);
    }
    golden!("galaxy_parameters", &golden.finish());

    client.close().await;
}

/// Run by [`the_golden_parameters_are_shared_by_every_universe_of_the_seed`] on its shared server,
/// after the golden, whose universe is of the same seed.
async fn two_universes_of_one_seed_share_their_galaxy_and_its_parameters(server: &TestServer) {
    let mut client = connected(server).await;
    let first = create(&mut client, "Kepler Reach: the first of two", SEED).await;
    let second = create(&mut client, "Talos", SEED).await;
    assert_ne!(first.id, second.id);

    let theirs = parameters(&mut client, &first.id).await.unwrap();
    let ours = parameters(&mut client, &second.id).await.unwrap();
    assert_eq!(theirs.groups, ours.groups);
    assert_eq!(theirs.seed, ours.seed);
    assert_eq!(
        ours.universe, second.id,
        "each answer names its own universe"
    );
    let galaxies = server.stats().galaxies();
    assert_eq!(
        (galaxies.builds(), galaxies.entries()),
        (1, 1),
        "one galaxy serves both saves: the cache is keyed by seed and version"
    );
    assert!(galaxies.bytes() > 0);

    client.close().await;
}

#[tokio::test]
async fn open_warms_the_galaxy_so_the_parameters_need_no_build() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach", SEED).await;
    assert_eq!(
        server.stats().galaxies().builds(),
        0,
        "creating a universe generates nothing"
    );

    open(&mut client, &universe.id).await;
    assert_eq!(
        server.stats().galaxies().builds(),
        1,
        "open warms the universe's galaxy (design note 6)"
    );
    parameters(&mut client, &universe.id).await.unwrap();
    let galaxies = server.stats().galaxies();
    assert_eq!(
        (galaxies.builds(), galaxies.hits()),
        (1, 1),
        "the parameters found the galaxy built"
    );

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn an_unknown_universe_is_refused_before_any_galaxy_is_built() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let error = parameters(&mut client, &UniverseIdHex::from_u64(0xdead))
        .await
        .unwrap_err();
    assert_eq!(error.code, hyperion_protocol::ErrorCode::UnknownUniverse);
    assert_eq!(error.field.as_deref(), Some("universe"));
    assert_eq!(server.stats().galaxies().builds(), 0);

    client.close().await;
    server.stop().await;
}
