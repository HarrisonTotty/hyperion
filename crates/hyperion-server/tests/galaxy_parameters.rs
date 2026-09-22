//! The galaxy parameters over a real socket: the response for a fixed seed, pinned, and what the
//! galaxy cache does behind it (plan 04, P04.T14.b).

mod common;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    CreateUniverseRequest, GalaxyParameters, GalaxyParametersRequest, OpenUniverseRequest,
    RequestBody, RequestError, ResponseBody, SeedHex, UniverseIdHex, UniverseInfo,
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

#[tokio::test]
async fn the_parameters_of_a_fixed_seed_are_the_golden_response() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let universe = create(&mut client, "Kepler Reach", SEED).await;
    let response = parameters(&mut client, &universe.id).await.unwrap();
    assert_eq!(response.seed, SeedHex::from_u64(SEED));
    assert_eq!(response.generator_version, GENERATOR_VERSION.get());

    // The response's own JSON, as the client receives it, under the golden header that ties it to
    // the generator version: a version bump re-blesses this file.
    let mut golden = GoldenWriter::new();
    golden.header(GENERATOR_VERSION.get());
    for line in serde_json::to_string_pretty(&response).unwrap().lines() {
        golden.line(line);
    }
    golden!("galaxy_parameters", &golden.finish());

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn two_universes_of_one_seed_share_their_galaxy_and_its_parameters() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let first = create(&mut client, "Kepler Reach", SEED).await;
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
    server.stop().await;
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
