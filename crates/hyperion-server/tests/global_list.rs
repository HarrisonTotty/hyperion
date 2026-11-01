//! Plan 10's global-list requests over a real socket (P10.T10.a): `global_features`, the galaxy's
//! centre, streams and dwarf cores with its accreted progenitors, and `stream_track`, one stream's
//! track, built for the request; a stream the list lacks refused naming `stream`.

mod common;

use common::origins::SEED;
use common::{TestClient, TestServer};
use hyperion_protocol::{
    ErrorCode, GlobalEntryWire, GlobalFeatures, GlobalFeaturesRequest, ProgenitorKindWire,
    RequestBody, ResponseBody, StreamTrackRequest, TrackStatusWire, TrackWire, UniverseIdHex,
};

/// A server, a client that has said hello, and a universe of [`SEED`].
async fn a_universe() -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Global List", SEED).await.id;
    (server, client, universe)
}

/// The universe's global list.
async fn global_features(client: &mut TestClient, universe: &UniverseIdHex) -> GlobalFeatures {
    let body = RequestBody::GlobalFeatures(GlobalFeaturesRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::GlobalFeatures(answer)) => answer,
        other => panic!("expected the global list, got {other:?}"),
    }
}

/// The list holds the centre first, then every stream and every core by number, each with its
/// shell; every stream's track is pending, as this server holds no tube; and the progenitors are
/// the last major merger, then the recent ones. A second request is answered from the list kept.
#[tokio::test]
async fn global_features_list_the_centre_streams_cores_and_progenitors() {
    let (server, mut client, universe) = a_universe().await;
    let answer = global_features(&mut client, &universe).await;
    assert_eq!(answer.universe, universe);
    let entries = &answer.entries;
    assert!(matches!(entries[0], GlobalEntryWire::GalacticCentre { .. }));
    let (mut streams, mut living, mut cores) = (0_u16, 0, 0);
    for entry in &entries[1..] {
        match entry {
            GlobalEntryWire::Stream {
                number,
                mass_msun,
                shell,
                progenitor,
                track,
            } => {
                assert_eq!(cores, 0, "every stream before every core");
                assert_eq!(*number, streams);
                assert_eq!(*track, TrackStatusWire::Pending);
                // A stream wholly beyond the root cube's corner has an empty shell, its inner
                // radius past its outer.
                assert!(*mass_msun > 0.0, "{entry:?}");
                assert!(
                    shell.inner_ly >= 0.0 && shell.outer_ly.is_finite(),
                    "{entry:?}"
                );
                streams += 1;
                living += usize::from(progenitor.is_some());
            }
            GlobalEntryWire::DwarfCore { number, .. } => {
                assert_eq!(*number, cores);
                cores += 1;
            }
            GlobalEntryWire::GalacticCentre { .. } => panic!("a second centre"),
        }
    }
    assert!(streams > 100, "{streams} streams");
    assert!(living > 0, "no living globular's stream");
    assert_eq!(
        answer.progenitors[0].kind,
        ProgenitorKindWire::DominantMerger
    );
    assert!(
        answer.progenitors[1..]
            .iter()
            .all(|p| p.kind == ProgenitorKindWire::Recent)
    );
    let again = global_features(&mut client, &universe).await;
    assert_eq!(again, answer);
    let held = server.stats().global_lists();
    assert_eq!(
        (held.entries(), held.hits()),
        (1, 1),
        "the second answered from the list kept"
    );
    client.close().await;
    server.stop().await;
}

/// A stream's track is ready, each arm at most 128 points with widths and a speed; a stream the
/// galaxy's list has no stream for is refused as `unknown_feature`, naming `stream`.
#[tokio::test]
async fn stream_track_answers_a_track_and_refuses_a_stream_the_list_lacks() {
    let (server, mut client, universe) = a_universe().await;
    let body = RequestBody::StreamTrack(StreamTrackRequest {
        universe: universe.clone(),
        stream: 0,
    });
    let answer = match client.request(body).await {
        Ok(ResponseBody::StreamTrack(answer)) => answer,
        other => panic!("expected a track, got {other:?}"),
    };
    assert_eq!((&answer.universe, answer.stream), (&universe, 0));
    let TrackWire::Ready { trailing, leading } = answer.track else {
        panic!("a track built for the request is ready");
    };
    for arm in [&trailing, &leading] {
        assert!((2..=128).contains(&arm.len()), "{} points", arm.len());
        for point in arm {
            assert!(point.normal_width_ly > 0.0 && point.binormal_width_ly > 0.0);
            assert!(point.speed_km_s > 0.0 && point.speed_km_s < 3_000.0);
        }
    }
    // Past the list's streams, and past any stream's number.
    for stream in [4_095, u16::MAX] {
        let beyond = RequestBody::StreamTrack(StreamTrackRequest {
            universe: universe.clone(),
            stream,
        });
        let refused = client.request(beyond).await.expect_err("no such stream");
        assert_eq!(refused.code, ErrorCode::UnknownFeature);
        assert_eq!(refused.field.as_deref(), Some("stream"));
    }
    client.close().await;
    server.stop().await;
}
