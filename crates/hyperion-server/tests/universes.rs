//! The universe lifecycle over a real socket: creating, listing and opening universes, and what
//! survives a restart (plan 04, P04.T14.a).

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::{TestClient, TestServer, test_entropy};
use hyperion_protocol::{
    CreateUniverseRequest, DensityMapRequest, ErrorCode, GalacticPosition, GalaxyParametersRequest,
    MapPopulation, MapView, MassLayer, OpenUniverseRequest, RequestBody, RequestError,
    ResponseBody, SeedHex, SystemsInRangeRequest, UniverseIdHex, UniverseInfo, UniverseList,
    UniverseStatus, UniverseTime,
};
use hyperion_server::limits::MAX_UNIVERSE_NAME_CHARS;
use hyperion_sim::GENERATOR_VERSION;

/// The `n`-th value the test server's entropy hands out, counting from 0.
fn drawn(n: usize) -> u64 {
    test_entropy()
        .nth(n)
        .expect("the test entropy has values to spare")
}

async fn create(
    client: &mut TestClient,
    name: &str,
    seed: Option<u64>,
) -> Result<UniverseInfo, RequestError> {
    let body = RequestBody::CreateUniverse(CreateUniverseRequest {
        name: name.to_owned(),
        seed: seed.map(SeedHex::from_u64),
    });
    client.request(body).await.map(|response| match response {
        ResponseBody::CreateUniverse(info) => info,
        other => panic!("expected the created universe, got {other:?}"),
    })
}

async fn list(client: &mut TestClient) -> UniverseList {
    match client.request(RequestBody::ListUniverses).await {
        Ok(ResponseBody::ListUniverses(list)) => list,
        other => panic!("expected the universe list, got {other:?}"),
    }
}

async fn open(client: &mut TestClient, id: &UniverseIdHex) -> Result<UniverseInfo, RequestError> {
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: id.clone(),
    });
    client.request(body).await.map(|response| match response {
        ResponseBody::OpenUniverse(info) => info,
        other => panic!("expected the opened universe, got {other:?}"),
    })
}

/// Every file under `root`, relative to it.
fn files_under(root: &Path) -> BTreeSet<PathBuf> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeSet<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }
    let mut files = BTreeSet::new();
    walk(root, root, &mut files);
    files
}

/// Where a universe's identity file lives, relative to the data directory.
fn save_file(id: &UniverseIdHex) -> PathBuf {
    Path::new("universes")
        .join(id.as_str())
        .join("universe.json")
}

async fn connected(server: &TestServer) -> TestClient {
    let mut client = server.connect().await;
    client.hello().await;
    client
}

#[tokio::test]
async fn a_universe_created_with_a_seed_is_listed_and_opens_with_the_same_info() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;

    let info = create(&mut client, "Kepler Reach", Some(0x4d2))
        .await
        .unwrap();
    assert_eq!(
        info,
        UniverseInfo {
            // A given seed draws nothing, so the ID is the first value drawn.
            id: UniverseIdHex::from_u64(drawn(0)),
            name: "Kepler Reach".to_owned(),
            seed: SeedHex::from_u64(0x4d2),
            generator_version: GENERATOR_VERSION.get(),
            status: UniverseStatus::Compatible,
        }
    );
    assert_eq!(
        list(&mut client).await,
        UniverseList {
            universes: vec![info.clone()],
            server_generator_version: GENERATOR_VERSION.get(),
        }
    );
    assert_eq!(open(&mut client, &info.id).await, Ok(info));

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_universe_created_without_a_seed_takes_the_value_drawn_first() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;

    let info = create(&mut client, "Talos", None).await.unwrap();
    assert_eq!(info.seed, SeedHex::from_u64(drawn(0)));
    assert_eq!(info.id, UniverseIdHex::from_u64(drawn(1)));

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_bad_name_is_a_bad_request_naming_the_field() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;

    let too_long = "a".repeat(MAX_UNIVERSE_NAME_CHARS + 1);
    for name in ["", "   ", &too_long, "Kepler\u{7}Reach"] {
        let error = create(&mut client, name, Some(1)).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::BadRequest, "{name:?}: {error:?}");
        assert_eq!(error.field.as_deref(), Some("name"), "{name:?}");
    }
    assert_eq!(list(&mut client).await.universes, []);
    assert_eq!(files_under(server.data_dir()), BTreeSet::new());

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_name_in_use_in_any_case_is_name_taken() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;

    create(&mut client, "Talos", Some(1)).await.unwrap();
    for name in ["Talos", " TALOS "] {
        let error = create(&mut client, name, Some(2)).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::NameTaken, "{name:?}: {error:?}");
        assert_eq!(error.field.as_deref(), Some("name"));
    }
    assert_eq!(list(&mut client).await.universes.len(), 1);

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn an_unknown_universe_is_refused() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;

    let error = open(&mut client, &UniverseIdHex::from_u64(0xdead))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::UnknownUniverse);
    assert_eq!(error.field.as_deref(), Some("universe"));
    assert_eq!(error.message, "no universe has id 000000000000dead");

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn universes_survive_a_restart_as_one_file_each() {
    let server = TestServer::start().await;
    assert_eq!(
        files_under(server.data_dir()),
        BTreeSet::new(),
        "starting writes nothing"
    );
    let mut client = connected(&server).await;
    let talos = create(&mut client, "Talos", Some(0x4d2)).await.unwrap();
    let vega = create(&mut client, "Vega", None).await.unwrap();
    let before = list(&mut client).await;
    client.close().await;

    let server = server.restart().await;
    let mut client = connected(&server).await;
    assert_eq!(list(&mut client).await, before);
    assert_eq!(
        files_under(server.data_dir()),
        BTreeSet::from([save_file(&talos.id), save_file(&vega.id)])
    );
    assert_eq!(open(&mut client, &talos.id).await, Ok(talos));
    assert_eq!(open(&mut client, &vega.id).await, Ok(vega));

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_save_from_another_generator_version_is_listed_as_a_mismatch_and_refused() {
    let server = TestServer::start().await;
    let mut client = connected(&server).await;
    let talos = create(&mut client, "Talos", Some(0x4d2)).await.unwrap();
    client.close().await;

    let path = server.data_dir().join(save_file(&talos.id));
    let other_version = GENERATOR_VERSION.get() + 1;
    let mut save: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).expect("the save is JSON");
    save["generator_version"] = other_version.into();
    fs::write(&path, serde_json::to_string_pretty(&save).unwrap()).unwrap();
    let edited = fs::read(&path).unwrap();

    let server = server.restart().await;
    let mut client = connected(&server).await;
    assert_eq!(
        list(&mut client).await.universes,
        [UniverseInfo {
            generator_version: other_version,
            status: UniverseStatus::GeneratorMismatch,
            ..talos.clone()
        }]
    );
    // Every request that names a universe is refused alike, and none of them generates anything.
    let refusals = [
        RequestBody::OpenUniverse(OpenUniverseRequest {
            universe: talos.id.clone(),
        }),
        RequestBody::GalaxyParameters(GalaxyParametersRequest {
            universe: talos.id.clone(),
        }),
        RequestBody::DensityMap(DensityMapRequest {
            universe: talos.id.clone(),
            view: MapView::FaceOn,
            population: MapPopulation::All,
            resolution: 128,
            bits: 8,
        }),
        RequestBody::SystemsInRange(SystemsInRangeRequest {
            universe: talos.id.clone(),
            centre: GalacticPosition::default(),
            radius_ly: 50.0,
            time: UniverseTime::default(),
            min_layer: MassLayer::A,
            limit: 1_000,
        }),
        // The universe is checked before any other field (design note 24), so a request wrong in
        // every other way too is still refused for the universe.
        RequestBody::SystemsInRange(SystemsInRangeRequest {
            universe: talos.id.clone(),
            centre: GalacticPosition {
                cell_ly: [65_536, 0, 0],
                offset_m: [0.0; 3],
            },
            radius_ly: 200_000.0,
            time: UniverseTime {
                seconds: 0,
                nanos: 1_000_000_000,
            },
            min_layer: MassLayer::A,
            limit: 0,
        }),
        RequestBody::DensityMap(DensityMapRequest {
            universe: talos.id.clone(),
            view: MapView::FaceOn,
            population: MapPopulation::All,
            resolution: 100,
            bits: 7,
        }),
    ];
    for body in refusals {
        let error = client.request(body).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::GeneratorVersionMismatch);
        assert_eq!(
            error.message,
            format!(
                "universe {} was created with generator version {other_version}, and this server \
                 runs generator version {}",
                talos.id.as_str(),
                GENERATOR_VERSION.get()
            )
        );
    }
    assert_eq!(
        (
            server.stats().galaxies().builds(),
            server.stats().cells().entries()
        ),
        (0, 0),
        "a universe this server cannot run generates nothing"
    );
    assert_eq!(
        fs::read(&path).unwrap(),
        edited,
        "the save is never rewritten"
    );

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_save_in_a_later_format_is_not_listed_and_refuses_to_open() {
    let server = TestServer::start().await;
    let id = UniverseIdHex::from_u64(0x0123_4567_89ab_cdef);
    let path = server.data_dir().join(save_file(&id));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{"format":2,"identity":{"id":"0123456789abcdef"}}"#,
    )
    .unwrap();

    let server = server.restart().await;
    let mut client = connected(&server).await;
    assert_eq!(list(&mut client).await.universes, []);
    let error = open(&mut client, &id).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::UnsupportedSaveFormat);
    assert_eq!(
        error.message,
        "universe 0123456789abcdef is saved in format 2, and this server reads format 1"
    );

    client.close().await;
    server.stop().await;
}
