//! Plan 10's global-list requests over a real socket (P10.T10.a and P10.T10.b): `global_features`,
//! the galaxy's centre, streams and dwarf cores with its accreted progenitors, and `stream_track`,
//! one stream's track, built for the request and kept, so that its status turns `ready`; a stream
//! the list lacks refused naming `stream`. The test servers do not warm the tables at an open, so
//! every track is `pending` until a request builds its table.

mod common;

use std::time::Duration;

use common::origins::{self, SEED};
use common::{TestClient, TestServer};
use hyperion_protocol::{
    ErrorCode, GalacticPosition, GlobalEntryWire, GlobalFeatures, GlobalFeaturesRequest,
    OpenUniverseRequest, ProgenitorKindWire, QueryModeDto, RequestBody, ResolveSystemRequest,
    ResponseBody, StreamTrackRequest, SystemIdHex, SystemSummaryRequest, TrackStatusWire,
    TrackWire, UniverseIdHex,
};
use hyperion_server::GlobalListWarmUp;
use hyperion_sim::galaxy::global_list::members::{
    GlobalMemberRecord, StreamBandShares, stream_cell_members,
};
use hyperion_sim::galaxy::global_list::{GlobalList, TubeTable};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::units::LightYears;

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
/// shell; every stream's track is pending, as no request has built a tube; and the progenitors are
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

/// The track status of stream `number` in `answer`.
fn track_of(answer: &GlobalFeatures, number: u16) -> TrackStatusWire {
    answer
        .entries
        .iter()
        .find_map(|entry| match entry {
            GlobalEntryWire::Stream {
                number: n, track, ..
            } if *n == number => Some(*track),
            _ => None,
        })
        .expect("the stream is listed")
}

/// A stream's track is ready, each arm at most 128 points with widths and a speed, and its table
/// is kept, so that its status in the list turns from `pending` to `ready` (P10.T10.b), and a
/// second request reads the table kept; a stream the galaxy's list has no stream for is refused
/// as `unknown_feature`, naming `stream`.
#[tokio::test]
async fn stream_track_answers_a_track_and_refuses_a_stream_the_list_lacks() {
    let (server, mut client, universe) = a_universe().await;
    let before = global_features(&mut client, &universe).await;
    assert_eq!(track_of(&before, 0), TrackStatusWire::Pending);
    let body = RequestBody::StreamTrack(StreamTrackRequest {
        universe: universe.clone(),
        stream: 0,
    });
    let answer = match client.request(body.clone()).await {
        Ok(ResponseBody::StreamTrack(answer)) => answer,
        other => panic!("expected a track, got {other:?}"),
    };
    let after = global_features(&mut client, &universe).await;
    assert_eq!(track_of(&after, 0), TrackStatusWire::Ready);
    assert_eq!(track_of(&after, 1), TrackStatusWire::Pending);
    let again = match client.request(body).await {
        Ok(ResponseBody::StreamTrack(again)) => again,
        other => panic!("expected a track, got {other:?}"),
    };
    assert_eq!(again, answer, "the kept table's track");
    let tables = server.stats().tube_tables();
    assert_eq!(
        (tables.entries(), tables.hits()),
        (1, 1),
        "one table built and kept, read once more"
    );
    assert_eq!((&answer.universe, answer.stream), (&universe, 0));
    let TrackWire::Ready {
        length_ly,
        trailing,
        leading,
    } = answer.track.clone()
    else {
        panic!("a track built for the request is ready");
    };
    assert!(length_ly.is_some_and(|l| l > 0.0), "{length_ly:?}");
    for arm in [&trailing, &leading] {
        assert!((2..=128).contains(&arm.len()), "{} points", arm.len());
        for point in arm {
            assert!(point.normal_width_ly > 0.0 && point.binormal_width_ly > 0.0);
            assert!(point.speed_km_s > 0.0 && point.speed_km_s < 3_000.0);
            assert!(point.dispersion_km_s.is_some_and(|d| d > 0.0), "{point:?}");
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

/// A member of band A of stream 0 of the universes' galaxy, from the cells about the middle of its
/// tube: the sim's own enumeration, whose record the server resolves its ID to.
fn a_stream_member() -> GlobalMemberRecord {
    let galaxy = origins::galaxy();
    let list = GlobalList::build(galaxy);
    let spec = &list.debris().streams()[0];
    let tube = TubeTable::build(galaxy, spec).expect("the full potential");
    let shares = StreamBandShares::of(galaxy, spec);
    let middle = tube.knots().nth(256).expect("a tube's 512 knots").centre();
    let mut members = Vec::new();
    tube.cells_touching(&middle, LightYears::new(200.0))
        .find_map(|cell| {
            stream_cell_members(galaxy, &tube, &shares, cell, MassBand::A, &mut members);
            members.first().copied()
        })
        .expect("a member about the middle of stream 0's tube")
}

/// P10.T10.b: a stream's member is summarised through the galaxy's list, now, with its stars,
/// and the table of its stream is built and kept for it; from generator version 27, which traces a
/// list member's motion on its line (P10.T10.b's registration), it is summarised observed too and
/// `resolve_system` places it, where below 27 both refused it.
#[tokio::test]
async fn a_stream_member_is_summarised_through_the_list_and_placed_on_its_line() {
    let member = a_stream_member();
    let (server, mut client, universe) = a_universe().await;
    let system = SystemIdHex::from_u64(member.record().id().raw());
    let summary = |mode| {
        RequestBody::SystemSummary(SystemSummaryRequest {
            universe: universe.clone(),
            system: system.clone(),
            time: origins::at_years(0),
            mode,
        })
    };
    match client.request(summary(QueryModeDto::Now)).await {
        Ok(ResponseBody::SystemSummary(answer)) => {
            assert_eq!(answer.system, system);
            assert!(!answer.stars.is_empty(), "{answer:?}");
        }
        other => panic!("expected the member's summary, got {other:?}"),
    }
    assert_eq!(
        server.stats().tube_tables().entries(),
        1,
        "stream 0's table kept"
    );
    let observer = QueryModeDto::Observed {
        observer: GalacticPosition {
            cell_ly: [0, 26_000, 0],
            offset_m: [0.0; 3],
        },
    };
    match client.request(summary(observer)).await {
        Ok(ResponseBody::SystemSummary(answer)) => assert_eq!(answer.system, system),
        other => panic!("expected the member's observed summary, got {other:?}"),
    }
    let resolve = RequestBody::ResolveSystem(ResolveSystemRequest {
        universe: universe.clone(),
        system: system.clone(),
        time: origins::at_years(0),
        mode: QueryModeDto::Now,
    });
    match client.request(resolve).await {
        Ok(ResponseBody::ResolveSystem(answer)) => assert_eq!(answer.record.id, system),
        other => panic!("expected the member placed on its line, got {other:?}"),
    }
    client.close().await;
    server.stop().await;
}

/// P10.T10.b: a server that warms at an open builds the galaxy's list for it, which the first
/// `global_features` finds, and its shutdown gives the warm-up up rather than waiting for every
/// table.
#[tokio::test]
async fn an_open_starts_the_warm_up_and_the_shutdown_gives_it_up() {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let server = TestServer::start_with(
        TestServer::config(data_dir.path())
            .global_list_warm_up(GlobalListWarmUp::AtOpen)
            .build(),
    )
    .await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Global List Warm-up", SEED).await.id;
    let open = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    assert!(matches!(
        client.request(open).await,
        Ok(ResponseBody::OpenUniverse(_))
    ));
    let answer = global_features(&mut client, &universe).await;
    assert!(answer.entries.len() > 100);
    assert_eq!(server.stats().global_lists().entries(), 1);
    client.close().await;
    server.stop_within(Duration::from_secs(60)).await;
}
