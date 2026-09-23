//! What one misbehaving client costs the server, and what two well-behaved ones see of each other
//! (plan 04, P04.T15).
//!
//! Every case here runs over a real socket against a real server: a flood of requests, a socket
//! closed with a map half built, an oversized frame, a run of malformed ones, two connections'
//! queries interleaved through the shared caches, and a shutdown with work still queued. The server
//! must answer every request exactly once, hold each client to the limits of
//! [`limits`](hyperion_server::limits), and come out of all of it able to serve the next client.
//!
//! The suite is accepted by passing ten times in a row, so nothing here is timed: a test that needs
//! work to be under way waits on
//! [`TestServer::stats_until`](common::TestServer::stats_until) until the server's own counters say
//! it is, and then asserts what they say. A test that only *assumed* the work was in flight would
//! pass whether the server stalled or not, which is the mistake a validator found in T14.c's stall
//! test.

mod common;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use common::{NETWORK_TIMEOUT, SHUTDOWN_TIMEOUT, TestClient, TestServer};
use hyperion_protocol::{
    ClientMessage, DensityMapRequest, ErrorCode, GalacticPosition, MapPopulation, MapView,
    MassLayer, OpenUniverseRequest, RequestBody, ResponseBody, ServerMessage, SystemsInRange,
    SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_server::limits::{
    MAX_CENSUS_LIMIT, MAX_CONSECUTIVE_MALFORMED_FRAMES, MAX_IN_FLIGHT_REQUESTS,
    MAX_INBOUND_FRAME_BYTES,
};

/// The seed of every universe these tests create, as the rest of the suite uses.
const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year: the wire carries a time as whole seconds.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// The WebSocket close code for a policy violation (RFC 6455, section 7.4.1).
const POLICY_VIOLATION: u16 = 1008;

/// One CPU worker, for the tests that need work to queue behind the job in hand.
fn one_worker() -> NonZeroUsize {
    NonZeroUsize::new(1).expect("one worker")
}

/// The terms of one `systems_in_range` request, so that a test changes only what it means to.
#[derive(Debug, Clone)]
struct Query {
    cell_ly: [i32; 3],
    radius_ly: f64,
    years: i64,
    min_layer: MassLayer,
    limit: u32,
}

impl Query {
    /// The cheapest query worth making: one light-year of the Sun-like point at the epoch, in the
    /// plane 26,000 ly out and clear of the bar.
    ///
    /// The tests that fire hundreds of these are about the in-flight cap and not about the query, and
    /// the query's cost goes with the cube of its radius: a 50 ly query at this centre visits some
    /// 1,700 cells and costs about 200 ms under `cargo test` (plan 04, the T14.d measurements).
    fn tiny() -> Self {
        Self {
            cell_ly: [0, 26_000, 0],
            radius_ly: 1.0,
            years: 0,
            min_layer: MassLayer::A,
            limit: MAX_CENSUS_LIMIT,
        }
    }

    /// Twenty light-years of `cell_ly` at the epoch, a query that returns systems and still costs
    /// well under a tenth of the 50 ly one.
    fn around(cell_ly: [i32; 3]) -> Self {
        Self {
            cell_ly,
            radius_ly: 20.0,
            years: 0,
            min_layer: MassLayer::A,
            limit: MAX_CENSUS_LIMIT,
        }
    }

    /// The same query `years` Julian years from the epoch.
    fn at_years(mut self, years: i64) -> Self {
        self.years = years;
        self
    }

    /// The same query over a radius of `radius_ly` light-years.
    fn with_radius(mut self, radius_ly: f64) -> Self {
        self.radius_ly = radius_ly;
        self
    }

    /// The same query wanting only `min_layer` and heavier, under a census limit of `limit`.
    fn above_layer(mut self, min_layer: MassLayer, limit: u32) -> Self {
        self.min_layer = min_layer;
        self.limit = limit;
        self
    }

    fn body(&self, universe: &UniverseIdHex) -> RequestBody {
        RequestBody::SystemsInRange(SystemsInRangeRequest {
            universe: universe.clone(),
            centre: GalacticPosition {
                cell_ly: self.cell_ly,
                offset_m: [0.0; 3],
            },
            radius_ly: self.radius_ly,
            time: UniverseTime {
                seconds: self.years * SECONDS_PER_JULIAN_YEAR,
                nanos: 0,
            },
            min_layer: self.min_layer,
            limit: self.limit,
        })
    }
}

/// A `density_map` request for the whole galaxy at `resolution` pixels and eight bits.
fn map_body(universe: &UniverseIdHex, view: MapView, resolution: u16) -> RequestBody {
    RequestBody::DensityMap(DensityMapRequest {
        universe: universe.clone(),
        view,
        population: MapPopulation::All,
        resolution,
        bits: 8,
    })
}

/// Opens `universe`, which builds its galaxy on the pool (P04.T14.a), so that the work a test waits
/// for afterwards is the work it means to wait for and not the galaxy build.
async fn open(client: &mut TestClient, universe: &UniverseIdHex) {
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
}

/// Reads `count` responses from `client` and returns their bodies by request ID.
///
/// A connection's terminal frames are queued in the order its requests finished and not in the order
/// they were made (P04.T13), so a test that has several in flight matches the answers by ID. Panics
/// on a refusal or any other message, so a test that expects one reads it itself.
async fn responses(client: &mut TestClient, count: usize) -> BTreeMap<u32, ResponseBody> {
    let mut bodies = BTreeMap::new();
    for _ in 0..count {
        match client.next_message().await {
            ServerMessage::Response { id, body } => {
                assert!(
                    bodies.insert(id.0, body).is_none(),
                    "request {} was answered twice",
                    id.0
                );
            }
            other => panic!("expected a response, got {other:?}"),
        }
    }
    bodies
}

/// The systems of a `systems_in_range` response, which is all these queries are answered with.
fn in_range(body: &ResponseBody) -> &SystemsInRange {
    match body {
        ResponseBody::SystemsInRange(answer) => answer,
        other => panic!("expected the systems in range, got {other:?}"),
    }
}

/// A response body as the bytes it takes on the wire, for comparing two answers exactly.
fn json(body: &ResponseBody) -> String {
    serde_json::to_string(body).expect("response bodies serialise")
}

/// Two hundred range queries fired without waiting end in exactly two hundred terminal messages,
/// most of them `too_many_requests`, and the connection answers `ping` throughout.
///
/// The universe is left cold, so the eight queries the in-flight cap admits are all still waiting for
/// their galaxy to be built — some 0.9 s under `cargo test` (`tests/common`'s `NETWORK_TIMEOUT`) —
/// while the remaining frames arrive in a few milliseconds. Every refusal is therefore the cap's,
/// which is asserted rather than assumed: no other code may appear. Measured here, 192 of the 200
/// are refused and the 8 the cap admits are answered. The `ping` goes out with the flood, before a
/// single answer is read, and the second one at the end is what proves there was no two hundred and
/// first terminal message.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_hundred_queries_fired_at_once_end_in_exactly_two_hundred_terminal_messages() {
    /// Queries fired without waiting for any of them.
    const FIRED: usize = 200;

    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let query = Query::tiny();

    let mut fired = Vec::with_capacity(FIRED);
    for _ in 0..FIRED {
        fired.push(client.send_request(query.body(&universe.id)).await);
    }
    // Sent with the flood and not after it: a connection whose reading stalled behind its own
    // requests would never answer this.
    client.send(&ClientMessage::Ping { nonce: 99 }).await;

    let mut ended: BTreeMap<u32, Option<ErrorCode>> = BTreeMap::new();
    let mut ponged = false;
    while ended.len() < FIRED || !ponged {
        let (id, code) = match client.next_message().await {
            ServerMessage::Response { id, .. } => (id, None),
            ServerMessage::RequestError { id, error } => (id, Some(error.code)),
            ServerMessage::Pong { nonce } => {
                assert_eq!(nonce, 99);
                assert!(!ponged, "the ping was answered twice");
                ponged = true;
                continue;
            }
            other => panic!("expected a terminal message or the pong, got {other:?}"),
        };
        assert!(
            ended.insert(id.0, code).is_none(),
            "request {} ended twice",
            id.0
        );
    }
    assert_eq!(
        ended.keys().copied().collect::<Vec<_>>(),
        fired.iter().map(|id| id.0).collect::<Vec<_>>(),
        "every request fired ended exactly once, and nothing else ended"
    );
    let refused = ended.values().filter(|code| code.is_some()).count();
    assert!(
        refused > 0,
        "the in-flight cap of {MAX_IN_FLIGHT_REQUESTS} refuses some of {FIRED} queries at once"
    );
    for (id, code) in &ended {
        assert!(
            matches!(code, None | Some(ErrorCode::TooManyRequests)),
            "request {id} ended as {code:?}, and only the in-flight cap may refuse one here"
        );
    }
    assert!(
        FIRED - refused >= MAX_IN_FLIGHT_REQUESTS,
        "the cap admits {MAX_IN_FLIGHT_REQUESTS} queries at a time, and {} were answered",
        FIRED - refused
    );
    // Nothing was left queued behind the two hundred: the next answer read is this pong.
    assert_eq!(
        client.ping(100).await,
        ServerMessage::Pong { nonce: 100 },
        "the server still answers ping after the flood"
    );

    let requests = server.stats().requests();
    let accepted = u64::try_from(FIRED - refused).expect("fewer than 2⁶⁴ queries") + 1;
    assert_eq!(
        (
            requests.in_flight(),
            requests.accepted(),
            requests.refused(),
            requests.responded(),
            requests.failed(),
            requests.cancelled(),
            requests.abandoned(),
        ),
        // The create was accepted and answered too; every accepted query answered, and none of them
        // failed, was cancelled or was left behind.
        (
            0,
            accepted,
            u64::try_from(refused).expect("fewer than 2⁶⁴ queries"),
            accepted,
            0,
            0,
            0
        )
    );

    client.close().await;
    server.stop().await;
}

/// Closing the socket with a 1,024-pixel map in flight leaves the pool with nothing queued.
///
/// One worker and the dearest raster there is: 32 bands, each of them seconds of work (plan 04,
/// "Measured map costs"), so the map cannot finish while the test runs. The galaxy is warmed first,
/// or the one worker would still be building it and no band would ever be queued; the pool's
/// counters are then read before the close, so that a map which never reached the pool fails this
/// test instead of letting it pass on an empty queue. Four range queries are then queued behind the
/// band in hand, because the two kinds of work stop by different routes: a band nobody waits for is
/// skipped because its flight is dropped with the last waiter, while a query's job is skipped only
/// because the closing connection cancels its request's token. After the close every job still
/// queued is skipped rather than computed, which the pool's `cancelled` counter shows — the 31
/// bands and the 4 queries, measured, against two jobs run: the galaxy and the band in hand. That
/// band runs to its end, which is a CPU wait and not a network one, so the drain is given
/// [`SHUTDOWN_TIMEOUT`].
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_a_socket_with_a_map_in_flight_leaves_the_pool_idle() {
    /// Range queries queued behind the map's band in hand when the socket closes.
    const QUERIES: usize = 4;

    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(one_worker())
        .build();
    let server = TestServer::start_with(config).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    open(&mut client, &universe.id).await;
    assert_eq!(server.stats().galaxies().builds(), 1, "the galaxy is warm");

    client
        .send_request(map_body(&universe.id, MapView::EdgeOn, 1_024))
        .await;
    let pool = server
        .stats_until("the map's bands are in the pool", |stats| {
            stats.pool().running() > 0 && stats.pool().queued_bulk() > 0
        })
        .await
        .pool();
    assert_eq!(
        pool.running(),
        1,
        "the one worker has a band in hand: {pool:?}"
    );
    let queued = u64::try_from(pool.queued_bulk()).expect("fewer than 2⁶⁴ bands");
    assert_eq!(
        server.stats().requests().in_flight(),
        1,
        "the map is the one request in flight"
    );
    // Queries wait behind the band in hand, since the one worker is busy with it for seconds.
    for _ in 0..QUERIES {
        client
            .send_request(Query::around([0, 26_000, 0]).body(&universe.id))
            .await;
    }
    server
        .stats_until("the queries are queued behind the band", |stats| {
            stats.pool().queued_interactive() == QUERIES
        })
        .await;

    // The client goes away with the map and the queries unanswered. The count is taken again here,
    // because a band that finished since the one above would have taken the next job out of the
    // queue.
    let at_close = server.stats().pool();
    let queued = u64::try_from(at_close.queued_bulk() + at_close.queued_interactive())
        .expect("fewer than 2⁶⁴ jobs")
        .min(queued + u64::try_from(QUERIES).expect("four"));
    client.close().await;
    let idle = server
        .stats_until_within(
            SHUTDOWN_TIMEOUT,
            "the connection has gone and the pool's queues are empty",
            |stats| {
                stats.connections() == 0
                    && stats.pool().queued_bulk() == 0
                    && stats.pool().queued_interactive() == 0
            },
        )
        .await;
    assert_eq!(
        (idle.requests().abandoned(), idle.requests().in_flight()),
        (1 + u64::try_from(QUERIES).expect("four"), 0),
        "the map and the queries were abandoned with their connection"
    );
    // The jobs still queued were skipped rather than computed. One of them may have reached a
    // worker in the instant between the count above and the close taking effect, so the count is
    // held to one less; that job is also why `completed` is allowed a third beside the galaxy build
    // and the band in hand. A query that ran would be a fourth.
    assert!(
        idle.pool().cancelled() + 1 >= queued,
        "the {queued} jobs still queued were skipped: {:?}",
        idle.pool()
    );
    assert!(
        idle.pool().completed() <= 3,
        "no job that nobody waited for was computed: {:?}",
        idle.pool()
    );
    assert_eq!(
        idle.maps().entries(),
        0,
        "a map nobody waits for is not cached"
    );

    server.stop().await;
}

/// Two clients asking for one map get the same frame, byte for byte, from one build of the raster.
///
/// The second client asks while the first one's raster is being rendered — one worker, and the
/// counters are read to be sure the bands are in the pool before the second request is sent — so the
/// two requests share one flight. The map cache's counters are what prove the single build: a miss
/// for each of the two callers, one more for the flight's own second look, no hit, and one entry
/// (plan 04, the T11.c deviations). Had the first map finished first, the second caller would have
/// been a hit and this test would fail rather than quietly prove nothing. Each client asks as its own
/// first request, so both answers carry request ID 1 and the frames are comparable as bytes.
///
/// The raster is 256 pixels, 16 bands and about a second of the one worker, so that the flight the
/// second client joins lasts far longer than the millisecond its request takes to arrive: at 128
/// pixels the whole raster was a few hundred milliseconds, which is a margin a loaded machine can eat.
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn two_clients_asking_for_one_map_get_the_same_bytes_from_one_build() {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(one_worker())
        .build();
    let server = TestServer::start_with(config).await;
    let mut setup = server.connected().await;
    let universe = setup.create_universe("Kepler Reach", SEED).await;
    open(&mut setup, &universe.id).await;

    let mut first = server.connected().await;
    let mut second = server.connected().await;
    let asked = first
        .send_request(map_body(&universe.id, MapView::FaceOn, 256))
        .await;
    server
        .stats_until("the map's bands are in the pool", |stats| {
            stats.pool().running() > 0 && stats.pool().queued_bulk() > 0
        })
        .await;
    let asked_again = second
        .send_request(map_body(&universe.id, MapView::FaceOn, 256))
        .await;
    assert_eq!(asked, asked_again, "both clients ask under the same ID");

    let one = first.next_text().await;
    let other = second.next_text().await;
    assert_eq!(
        one, other,
        "the two clients are sent the same frame, byte for byte"
    );

    let stats = server.stats();
    assert_eq!(
        stats.galaxies().builds(),
        1,
        "one galaxy, built by the open"
    );
    let maps = stats.maps();
    assert_eq!(
        (maps.entries(), maps.hits(), maps.misses()),
        (1, 0, 3),
        "one raster was rendered for both clients: {maps:?}"
    );

    for client in [first, second, setup] {
        client.close().await;
    }
    server.stop().await;
}

/// A 17 KiB frame closes the connection, and the request it had in flight is abandoned.
///
/// The frame is one KiB over [`MAX_INBOUND_FRAME_BYTES`], so the read fails and the server closes
/// without a close frame of its own: sending 1009 would need a direct tungstenite dependency pinned
/// to axum's, and the plan asks only for the close (plan 04, "Slow readers, for T15"). The query in
/// flight is on a cold universe, and the wait pins that it is still waiting for its galaxy — a
/// request in flight, a job running and no galaxy built yet — rather than assuming it from the
/// build's 0.9 s, so the abandonment it asserts is real: it is counted, and the client is never told.
/// Another client then gets the answer the first one threw away.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_frame_closes_the_connection_and_abandons_its_requests() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    let query = Query::tiny();
    client.send_request(query.body(&universe.id)).await;
    server
        .stats_until(
            "the query is in flight and its galaxy still building",
            |stats| {
                stats.requests().in_flight() == 1
                    && stats.pool().running() > 0
                    && stats.galaxies().builds() == 0
            },
        )
        .await;

    client
        .send_raw(&"x".repeat(MAX_INBOUND_FRAME_BYTES + 1_024))
        .await;
    assert_eq!(
        client.closed().await,
        None,
        "a frame the server cannot read closes the connection without a close frame"
    );
    let closed = server
        .stats_until("the abandoned query has been counted", |stats| {
            stats.connections() == 0 && stats.requests().in_flight() == 0
        })
        .await;
    assert_eq!(
        (closed.requests().abandoned(), closed.requests().responded()),
        (1, 1),
        "the query was abandoned, and only the create was answered"
    );

    // The server is unharmed, and answers the query the flooding client gave up on.
    let mut other = server.connected().await;
    let answer = other.request(query.body(&universe.id)).await;
    assert!(
        matches!(answer, Ok(ResponseBody::SystemsInRange(_))),
        "{answer:?}"
    );
    other.close().await;
    server.stop().await;
}

/// Sixteen malformed frames in a row close the connection with a policy violation, and the request
/// they arrived behind is abandoned.
///
/// [`MAX_CONSECUTIVE_MALFORMED_FRAMES`] is the count, and each frame is answered with the
/// connection-level `error` until the last one closes the socket. As in the oversized case the wait
/// pins that the query is still waiting for its galaxy before the abuse starts. The sixteen frames
/// and their answers are a few milliseconds against the galaxy build's 0.9 s, so a terminal message
/// among them would mean the server had answered a request it should still be running, and the loop
/// fails on one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sixteen_malformed_frames_close_the_connection_and_abandon_its_requests() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    client.send_request(Query::tiny().body(&universe.id)).await;
    server
        .stats_until(
            "the query is in flight and its galaxy still building",
            |stats| {
                stats.requests().in_flight() == 1
                    && stats.pool().running() > 0
                    && stats.galaxies().builds() == 0
            },
        )
        .await;

    for frame in 0..MAX_CONSECUTIVE_MALFORMED_FRAMES {
        client.send_raw("not json").await;
        match client.next_message().await {
            ServerMessage::Error { message } => {
                assert!(message.starts_with("malformed message: "), "{message}");
            }
            other => panic!("expected the connection-level error for frame {frame}, got {other:?}"),
        }
    }
    assert_eq!(
        client.closed().await,
        Some(POLICY_VIOLATION),
        "the sixteenth malformed frame closes the connection"
    );
    let closed = server
        .stats_until("the abandoned query has been counted", |stats| {
            stats.connections() == 0 && stats.requests().in_flight() == 0
        })
        .await;
    assert_eq!(
        (closed.requests().abandoned(), closed.requests().responded()),
        (1, 1),
        "the query was abandoned, and only the create was answered"
    );

    let mut other = server.connected().await;
    assert_eq!(other.ping(1).await, ServerMessage::Pong { nonce: 1 });
    other.close().await;
    server.stop().await;
}

/// Queries interleaved from two connections answer exactly as the same queries do run alone.
///
/// Four queries that overlap in the cells they visit, so that they share the cell cache and the order
/// they run in could matter: three around one centre — at the epoch, a century later, and wanting
/// only the heavier layers under a small limit — and one nearby. They are run alone on a cold server,
/// then all four at once from two connections on another cold server, and then alone again on that
/// warm server. Every answer is the same bytes all three times: the caches may make an answer cheaper
/// and may not change it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn interleaved_queries_from_two_connections_answer_as_they_do_alone() {
    let queries = [
        Query::around([0, 26_000, 0]),
        Query::around([0, 26_000, 0]).at_years(100),
        Query::around([500, 25_800, 100]),
        Query::around([0, 26_000, 0]).above_layer(MassLayer::C, 50),
    ];

    // Alone, one after another, on a server whose caches start cold.
    let (alone, alone_universe) = {
        let server = TestServer::start().await;
        let mut client = server.connected().await;
        let universe = client.create_universe("Kepler Reach", SEED).await;
        let mut answers = Vec::with_capacity(queries.len());
        for query in &queries {
            let body = one_answer(&mut client, query, &universe.id).await;
            assert!(
                !in_range(&body).systems.is_empty(),
                "a query that returns nothing would compare two empty answers"
            );
            answers.push(json(&body));
        }
        client.close().await;
        server.stop().await;
        (answers, universe.id)
    };

    // Interleaved, on another cold server: all four in flight at once, two to a connection, in an
    // order no single client would produce.
    let server = TestServer::start().await;
    let mut setup = server.connected().await;
    let universe = setup.create_universe("Kepler Reach", SEED).await;
    assert_eq!(
        universe.id, alone_universe,
        "an answer carries the universe's ID, so the two servers must draw the same one from \
         `test_entropy` or the bytes below would differ for that reason alone"
    );
    let mut first = server.connected().await;
    let mut second = server.connected().await;
    let third_query = first.send_request(queries[2].body(&universe.id)).await;
    let first_query = second.send_request(queries[0].body(&universe.id)).await;
    let fourth_query = first.send_request(queries[3].body(&universe.id)).await;
    let second_query = second.send_request(queries[1].body(&universe.id)).await;
    let from_first = responses(&mut first, 2).await;
    let from_second = responses(&mut second, 2).await;
    for (index, id, read) in [
        (2, third_query, &from_first),
        (3, fourth_query, &from_first),
        (0, first_query, &from_second),
        (1, second_query, &from_second),
    ] {
        let answer = read
            .get(&id.0)
            .unwrap_or_else(|| panic!("no answer to request {}", id.0));
        assert_eq!(
            json(answer),
            alone[index],
            "query {index} interleaved answers as it does alone"
        );
    }
    assert!(
        server.stats().cells().entries() > 0,
        "the queries filled the server's shared cell cache"
    );

    // And once more alone, on the warm caches the four just filled.
    let mut third = server.connected().await;
    for (index, query) in queries.iter().enumerate() {
        let body = one_answer(&mut third, query, &universe.id).await;
        assert_eq!(
            json(&body),
            alone[index],
            "query {index} answers the same from the warm caches"
        );
    }
    assert!(
        server.stats().cells().hits() > 0,
        "the warm run read cells the interleaved one generated"
    );

    for client in [first, second, third, setup] {
        client.close().await;
    }
    server.stop().await;
}

/// Asks one query and waits for its answer, which is matched to the request it answers.
async fn one_answer(
    client: &mut TestClient,
    query: &Query,
    universe: &UniverseIdHex,
) -> ResponseBody {
    let id = client.send_request(query.body(universe)).await;
    responses(client, 1)
        .await
        .remove(&id.0)
        .expect("the answer to the query just asked")
}

/// `Server::shutdown` returns within [`NETWORK_TIMEOUT`] with work queued and clients not reading.
///
/// One worker, so that a map's bands wait in the bulk queue and a second connection's eight queries
/// wait in the interactive one; both queues are read before the shutdown, and again as it starts, so
/// a run that queued nothing fails instead of proving nothing. Neither client reads or closes, so the
/// shutdown also pays each connection's
/// [`CLOSE_TIMEOUT`](hyperion_server::limits::CLOSE_TIMEOUT) before dropping its socket. What it may
/// not do is wait for the queues: it drops what is queued and joins the worker over the one job in
/// hand. Measured at 1.0 s against the 20 s bound, nearly all of it the two clients' close timeout.
///
/// The queries are the 50 ly one, some 120 to 200 ms of a worker each under `cargo test` (plan 04,
/// the T14.d measurements), because the state this test needs must not be a transient: eight of them
/// hold the interactive queue, and the bands behind it, for about a second, where eight
/// light-year-wide queries emptied both queues in a few tens of milliseconds and a loaded machine
/// could miss the state altogether. Only the query in hand is ever run, since the shutdown drops the
/// rest.
///
/// A 1,024-pixel edge-on map would instead leave a band of some seconds in the worker's hands, which
/// is the case `SHUTDOWN_TIMEOUT` is sized for (plan 04, T14.d) and not the queued work this case is
/// about.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_returns_within_the_network_timeout_with_work_queued() {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .workers(one_worker())
        .build();
    let server = TestServer::start_with(config).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await;
    open(&mut client, &universe.id).await;

    client
        .send_request(map_body(&universe.id, MapView::FaceOn, 128))
        .await;
    // The in-flight cap is per connection, so the queries come from a second one.
    let mut other = server.connected().await;
    let query = Query::around([0, 26_000, 0]).with_radius(50.0);
    for _ in 0..MAX_IN_FLIGHT_REQUESTS {
        other.send_request(query.body(&universe.id)).await;
    }
    let busy = server
        .stats_until("both of the pool's queues hold work", |stats| {
            stats.pool().running() > 0
                && stats.pool().queued_bulk() > 0
                && stats.pool().queued_interactive() > 0
        })
        .await;
    assert!(
        busy.requests().in_flight() >= 2,
        "a queued band and a queued query mean the map and a query are both in flight: {:?}",
        busy.requests()
    );

    // Still queued as the shutdown starts, and not only a moment before it.
    let at_shutdown = server.stats().pool();
    assert!(
        at_shutdown.queued_interactive() > 0 && at_shutdown.queued_bulk() > 0,
        "both queues still hold work as the shutdown starts: {at_shutdown:?}"
    );
    // The clients are still connected, and still not reading, while the shutdown runs. The bound is
    // the assertion: `stop_within` panics if the shutdown does not finish inside it.
    server.stop_within(NETWORK_TIMEOUT).await;
    drop((client, other));
}
