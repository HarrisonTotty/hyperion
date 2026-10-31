//! Plan 10's global-list requests over a real socket (P10.T10.a): `global_features`, the galaxy's
//! centre, streams and dwarf cores with its accreted progenitors, and `stream_track`, one stream's
//! track, built for the request; a stream the list lacks refused naming `stream`.

mod common;

use common::origins::SEED;
use common::{TestClient, TestServer};
use hyperion_protocol::{
    ErrorCode, FeatureKindWire, GlobalFeatures, GlobalFeaturesRequest, ProgenitorKindWire,
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
    assert_eq!(entries[0].kind, FeatureKindWire::GalacticCentre);
    assert_eq!(entries[0].number, None);
    let streams: Vec<_> = entries
        .iter()
        .filter(|e| e.kind == FeatureKindWire::Stream)
        .collect();
    assert!(streams.len() > 100, "{} streams", streams.len());
    for (k, stream) in streams.iter().enumerate() {
        assert_eq!(stream.number, Some(u16::try_from(k).expect("small")));
        assert_eq!(stream.track, Some(TrackStatusWire::Pending));
        // A stream wholly beyond the root cube's corner has an empty shell, its inner radius past
        // its outer.
        assert!(stream.mass_msun > 0.0, "{stream:?}");
        assert!(
            stream.shell_inner_ly >= 0.0 && stream.shell_outer_ly.is_finite(),
            "{stream:?}"
        );
    }
    assert!(
        streams.iter().any(|s| s.progenitor.is_some()),
        "no living globular's stream"
    );
    let kinds: Vec<_> = entries.iter().map(|e| e.kind).collect();
    let first_core = kinds.iter().position(|&k| k == FeatureKindWire::DwarfCore);
    if let Some(first) = first_core {
        assert!(
            kinds[first..]
                .iter()
                .all(|&k| k == FeatureKindWire::DwarfCore)
        );
    }
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
    assert_eq!(server.stats().global_lists().entries(), 1);
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
    let beyond = RequestBody::StreamTrack(StreamTrackRequest {
        universe,
        stream: 4_095,
    });
    let refused = client.request(beyond).await.expect_err("no such stream");
    assert_eq!(refused.code, ErrorCode::UnknownFeature);
    assert_eq!(refused.field.as_deref(), Some("stream"));
    client.close().await;
    server.stop().await;
}
