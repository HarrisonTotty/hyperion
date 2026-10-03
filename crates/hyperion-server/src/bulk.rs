//! Outbound binary frames: bulk payloads a request is answered with, cut into chunks under a fixed
//! header (rendering plan R03, R03.T10.a; Design notes 10 and 11).
//!
//! A request answered in bulk sends every chunk in order and then its terminal JSON `response`,
//! whose body carries the payload's manifest (`BulkManifestDto`: the chunk count and total
//! bytes), so plan 04's rule that parts precede the terminal response holds. Each frame is at most
//! [`MAX_BINARY_FRAME_BYTES`], header included, and starts with the 24-byte header documented in
//! `hyperion_protocol`'s crate docs, little-endian:
//!
//! | Bytes | Field |
//! | ----- | ----- |
//! | 0–3   | the magic `HYPB` |
//! | 4     | the format, 1 |
//! | 5     | reserved, 0 |
//! | 6–7   | the header's length, 24 |
//! | 8–11  | the request's ID |
//! | 12–15 | the chunk's index, from 0 |
//! | 16–19 | the chunk count |
//! | 20–23 | the payload's length in this frame |
//!
//! There is no checksum: TCP and the WebSocket framing already guard the bytes, and the count and
//! length checks catch the server's own bugs. Streaming the chunks under the outbound budget is
//! R03.T10.b's.

use std::error::Error;
use std::fmt;

use axum::body::Bytes;
use hyperion_protocol::{BulkManifestDto, RequestId, ResponseBody};

use crate::limits::MAX_BINARY_FRAME_BYTES;

mod sky;

/// The first four bytes of every binary frame.
pub(crate) const MAGIC: [u8; 4] = *b"HYPB";

/// The header's format.
pub(crate) const FORMAT: u8 = 1;

/// The header's length in bytes.
pub(crate) const HEADER_BYTES: usize = 24;

/// The payload bytes one frame carries at most: [`MAX_BINARY_FRAME_BYTES`] less the header,
/// 262,120.
pub(crate) const CHUNK_PAYLOAD_BYTES: usize = MAX_BINARY_FRAME_BYTES - HEADER_BYTES;

/// One binary frame's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BinaryFrameHeader {
    /// The request the chunk answers.
    pub(crate) request: RequestId,
    /// The chunk's index, from 0.
    pub(crate) index: u32,
    /// How many chunks the payload has.
    pub(crate) count: u32,
    /// The payload bytes in this frame.
    pub(crate) payload_len: u32,
}

/// The header's 24 bytes, little-endian, as the crate docs lay them out.
#[must_use]
pub(crate) fn encode_header(header: &BinaryFrameHeader) -> [u8; HEADER_BYTES] {
    let mut bytes = [0; HEADER_BYTES];
    bytes[0..4].copy_from_slice(&MAGIC);
    bytes[4] = FORMAT;
    bytes[5] = 0;
    let header_len = u16::try_from(HEADER_BYTES).expect("the header's 24 bytes fit a u16");
    bytes[6..8].copy_from_slice(&header_len.to_le_bytes());
    bytes[8..12].copy_from_slice(&header.request.0.to_le_bytes());
    bytes[12..16].copy_from_slice(&header.index.to_le_bytes());
    bytes[16..20].copy_from_slice(&header.count.to_le_bytes());
    bytes[20..24].copy_from_slice(&header.payload_len.to_le_bytes());
    bytes
}

/// A payload too large for its chunks to be counted in the header's `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "R06 and R09 add the first kinds answered in bulk")
)]
pub(crate) struct BuildBulkPayloadError {
    bytes: usize,
}

impl fmt::Display for BuildBulkPayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a bulk payload of {} bytes needs more chunks than a u32 counts",
            self.bytes
        )
    }
}

impl Error for BuildBulkPayloadError {}

/// A payload a request is answered with in bulk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BulkPayload {
    bytes: Bytes,
    chunks: u32,
}

impl BulkPayload {
    /// The payload `bytes`.
    ///
    /// # Errors
    ///
    /// [`BuildBulkPayloadError`] if its chunks cannot be counted in a `u32`, about a petabyte.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "R06 and R09 add the first kinds answered in bulk")
    )]
    pub(crate) fn new(bytes: Bytes) -> Result<Self, BuildBulkPayloadError> {
        let chunks = u32::try_from(bytes.len().div_ceil(CHUNK_PAYLOAD_BYTES))
            .map_err(|_| BuildBulkPayloadError { bytes: bytes.len() })?;
        Ok(Self { bytes, chunks })
    }

    /// The manifest the terminal response carries: the chunk count and the total bytes.
    #[must_use]
    pub(crate) fn manifest(&self) -> BulkManifestDto {
        BulkManifestDto {
            chunks: self.chunks,
            bytes: u64::try_from(self.bytes.len()).expect("a usize fits a u64 on every target"),
        }
    }

    /// The payload's frames for `request`: each a header and up to [`CHUNK_PAYLOAD_BYTES`] of the
    /// payload, in order; none for an empty payload.
    pub(crate) fn frames(&self, request: RequestId) -> impl Iterator<Item = Bytes> + use<> {
        chunk(self.bytes.clone(), request)
    }
}

/// `payload` cut into frames for `request`, each [`MAX_BINARY_FRAME_BYTES`] at most with its
/// header, in order: ⌈L ÷ 262,120⌉ of them for a payload of L bytes, none for an empty one.
///
/// # Panics
///
/// If the chunks cannot be counted in a `u32`, which [`BulkPayload::new`] rules out.
pub(crate) fn chunk(payload: Bytes, request: RequestId) -> impl Iterator<Item = Bytes> {
    let count = u32::try_from(payload.len().div_ceil(CHUNK_PAYLOAD_BYTES))
        .expect("a bulk payload's chunks fit a u32");
    (0..count).map(move |index| {
        let start = usize::try_from(index).expect("a u32 fits a usize here") * CHUNK_PAYLOAD_BYTES;
        let end = payload.len().min(start + CHUNK_PAYLOAD_BYTES);
        let body = payload.slice(start..end);
        let header = BinaryFrameHeader {
            request,
            index,
            count,
            payload_len: u32::try_from(body.len()).expect("a chunk is below 2³² bytes"),
        };
        let mut frame = Vec::with_capacity(HEADER_BYTES + body.len());
        frame.extend_from_slice(&encode_header(&header));
        frame.extend_from_slice(&body);
        Bytes::from(frame)
    })
}

/// What a handler answers a request with: the response's body, and a bulk payload whose chunks
/// precede it, if the request's kind asks for bulk.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Answer {
    /// The terminal response's body.
    pub(crate) body: ResponseBody,
    /// The chunks sent before it.
    pub(crate) bulk: Option<BulkPayload>,
}

impl From<ResponseBody> for Answer {
    fn from(body: ResponseBody) -> Self {
        Self { body, bulk: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::BULK_QUEUED_BYTES;
    use crate::limits::TCP_NOTSENT_LOWAT_BYTES;
    use crate::requests::Handlers;
    use crate::testing::Harness;

    /// A payload of `len` bytes, each its index's low byte, so that a misplaced chunk shows.
    fn payload(len: usize) -> Bytes {
        Bytes::from(
            (0..len)
                .map(|index| u8::try_from(index % 251).expect("below 251"))
                .collect::<Vec<u8>>(),
        )
    }

    /// The header's bytes for a pinned request, byte for byte; R03.T11 repeats this in
    /// TypeScript.
    #[test]
    fn the_header_is_pinned_byte_for_byte() {
        let header = BinaryFrameHeader {
            request: RequestId(0x0102_0304),
            index: 2,
            count: 61,
            payload_len: 262_120,
        };
        assert_eq!(
            encode_header(&header),
            [
                b'H', b'Y', b'P', b'B', // magic
                1, 0, // format, reserved
                24, 0, // header length
                0x04, 0x03, 0x02, 0x01, // request
                2, 0, 0, 0, // index
                61, 0, 0, 0, // count
                0xe8, 0xff, 0x03, 0x00, // payload length, 262,120
            ]
        );
    }

    #[test]
    fn a_payload_is_cut_into_frames_of_at_most_256_kib() {
        let request = RequestId(7);
        for (len, chunks) in [
            (0, 0),
            (1, 1),
            (262_120, 1),
            (262_121, 2),
            (15_000_000, 58),
            (15_728_640, 61),
        ] {
            let bytes = payload(len);
            let built = BulkPayload::new(bytes.clone()).expect("a payload of megabytes");
            assert_eq!(
                built.manifest(),
                BulkManifestDto {
                    chunks,
                    bytes: u64::try_from(len).unwrap(),
                },
                "{len} bytes"
            );
            let frames: Vec<Bytes> = built.frames(request).collect();
            assert_eq!(
                frames.len(),
                usize::try_from(chunks).unwrap(),
                "{len} bytes"
            );
            let mut rejoined = Vec::with_capacity(len);
            for (index, frame) in frames.iter().enumerate() {
                assert!(frame.len() <= MAX_BINARY_FRAME_BYTES, "{len} bytes");
                let expected = encode_header(&BinaryFrameHeader {
                    request,
                    index: u32::try_from(index).unwrap(),
                    count: chunks,
                    payload_len: u32::try_from(frame.len() - HEADER_BYTES).unwrap(),
                });
                assert_eq!(
                    frame[..HEADER_BYTES],
                    expected,
                    "{len} bytes, frame {index}"
                );
                rejoined.extend_from_slice(&frame[HEADER_BYTES..]);
            }
            assert_eq!(Bytes::from(rejoined), bytes, "{len} bytes rejoined");
        }
    }

    #[test]
    fn a_json_answer_carries_no_bulk() {
        let answer = Answer::from(ResponseBody::SceneCameras);
        assert_eq!(answer.body, ResponseBody::SceneCameras);
        assert_eq!(answer.bulk, None);
    }

    /// Design note 11: the unit harness's accepted sockets have the low-water mark set.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[tokio::test]
    async fn the_unit_harness_s_accepted_socket_has_the_low_water_mark() {
        let harness = Harness::start(Handlers).await;
        let mut client = harness.connect().await;
        client.hello().await;
        assert_eq!(harness.accepted_lowat(), TCP_NOTSENT_LOWAT_BYTES);
        client.close().await;
        harness.stop().await;
    }

    /// A request answered with `payload` in bulk as request 1, its chunks read to the end: the
    /// frames before the terminal message, and the terminal message.
    mod streaming {
        use std::time::Duration;

        use hyperion_protocol::{
            ErrorCode, SceneClockDto, SceneClockStateDto, SceneStateDto, ServerMessage,
            SubscriptionState, UniverseTime,
        };

        use super::*;
        use crate::limits::OUTBOUND_BYTES;
        use crate::subscriptions::{PendingPush, ScenePush};
        use crate::testing::{Client, Received, Scripted, Tap, WAIT, body, small_response};
        use crate::ws::ConnectionLimits;

        /// A field-sized payload: 15 × 10⁶ bytes, 58 chunks.
        const FIELD_BYTES: usize = 15_000_000;

        /// The header a bulk frame starts with, read.
        fn header(frame: &[u8]) -> BinaryFrameHeader {
            let word = |at: usize| u32::from_le_bytes(frame[at..at + 4].try_into().unwrap());
            assert_eq!(&frame[0..4], &MAGIC);
            assert_eq!(frame[4], FORMAT);
            assert_eq!(
                usize::from(u16::from_le_bytes([frame[6], frame[7]])),
                HEADER_BYTES
            );
            BinaryFrameHeader {
                request: RequestId(word(8)),
                index: word(12),
                count: word(16),
                payload_len: word(20),
            }
        }

        /// Reads until the terminal message of request `id`: the chunks before it and it.
        async fn read_to_the_end(client: &mut Client, id: u32) -> (Vec<Vec<u8>>, ServerMessage) {
            let mut chunks = Vec::new();
            loop {
                match client.next_frame().await {
                    Received::Binary(frame) => chunks.push(frame),
                    Received::Message(
                        message @ (ServerMessage::Response {
                            id: RequestId(answered),
                            ..
                        }
                        | ServerMessage::RequestError {
                            id: RequestId(answered),
                            ..
                        }),
                    ) if answered == id => return (chunks, message),
                    Received::Message(_) => {}
                }
            }
        }

        #[tokio::test]
        async fn every_chunk_precedes_the_terminal_response_in_order_and_one_is_queued_at_a_time() {
            let (handler, mut calls) = Scripted::new();
            let harness = Harness::start(handler).await;
            let mut client = harness.connect().await;
            client.hello().await;
            client.request(1, body(1)).await;
            let sent = payload(FIELD_BYTES);
            let bulk = BulkPayload::new(sent.clone()).unwrap();
            assert!(calls.next().await.respond_bulk(small_response(), bulk));
            let (chunks, terminal) = read_to_the_end(&mut client, 1).await;
            assert!(
                matches!(terminal, ServerMessage::Response { .. }),
                "{terminal:?}"
            );
            assert_eq!(chunks.len(), 58);
            let mut rejoined = Vec::with_capacity(FIELD_BYTES);
            for (index, frame) in chunks.iter().enumerate() {
                let header = header(frame);
                assert_eq!(
                    (header.request, header.index, header.count),
                    (RequestId(1), u32::try_from(index).unwrap(), 58)
                );
                assert_eq!(
                    frame.len(),
                    HEADER_BYTES + usize::try_from(header.payload_len).unwrap()
                );
                assert!(frame.len() <= MAX_BINARY_FRAME_BYTES);
                rejoined.extend_from_slice(&frame[HEADER_BYTES..]);
            }
            assert_eq!(rejoined, sent.to_vec());
            let outbound = harness.server().stats().outbound();
            // One full chunk at a time, every chunk but the last being full.
            assert_eq!(outbound.largest_bulk_queue_bytes(), MAX_BINARY_FRAME_BYTES);
            const { assert!(MAX_BINARY_FRAME_BYTES <= BULK_QUEUED_BYTES) };
            assert_eq!(harness.server().stats().requests().in_flight(), 0);
            client.close().await;
            harness.stop().await;
        }

        /// Two answers in bulk: the second's chunks follow the first's terminal response, and a
        /// cancel of the second, while the first streams, drops the second alone.
        #[tokio::test]
        async fn two_bulk_answers_stream_one_after_the_other_and_a_cancel_drops_only_its_own() {
            let (handler, mut calls) = Scripted::new();
            let harness = Harness::start(handler).await;
            let mut client = harness.connect_slow_reader().await;
            client.hello().await;
            for id in 1..=3 {
                client.request(id, body(id)).await;
            }
            // Three chunks each, answered in the order asked.
            for _ in 1..=3 {
                let call = calls.next().await;
                let bulk = BulkPayload::new(payload(3 * CHUNK_PAYLOAD_BYTES)).unwrap();
                assert!(call.respond_bulk(small_response(), bulk));
            }
            harness
                .requests_until(|counters| counters.in_flight() == 3)
                .await;
            client.cancel(2).await;
            let mut order = Vec::new();
            let mut ended = Vec::new();
            while ended.len() < 3 {
                match client.next_frame().await {
                    Received::Binary(frame) => order.push(header(&frame).request.0),
                    Received::Message(ServerMessage::Response { id, .. }) => {
                        ended.push((id.0, None));
                        order.push(100 + id.0);
                    }
                    Received::Message(ServerMessage::RequestError { id, error }) => {
                        ended.push((id.0, Some(error.code)));
                    }
                    Received::Message(other) => panic!("unexpected {other:?}"),
                }
            }
            assert!(
                ended.contains(&(2, Some(ErrorCode::Cancelled))),
                "{ended:?}"
            );
            // The first's chunks and response, then the third's: nothing interleaved, and none of
            // the second's after its own chunks stopped.
            let first_end = order.iter().position(|&tag| tag == 101).unwrap();
            assert!(order[..first_end].iter().all(|&tag| tag == 1), "{order:?}");
            let rest: Vec<u32> = order[first_end + 1..]
                .iter()
                .copied()
                .filter(|&tag| tag != 2)
                .collect();
            assert_eq!(rest, vec![3, 3, 3, 103], "{order:?}");
            client.close().await;
            harness.stop().await;
        }

        #[tokio::test]
        async fn a_scene_push_issued_mid_transfer_arrives_before_the_transfer_ends() {
            let (handler, mut calls, mut openings) = Scripted::with_openings();
            let harness = Harness::start(handler).await;
            let mut client = harness.connect_slow_reader().await;
            client.hello().await;
            client
                .request(
                    1,
                    hyperion_protocol::RequestBody::Subscribe(
                        hyperion_protocol::SubscribeRequest {
                            universe: hyperion_protocol::UniverseIdHex::from_u64(42),
                            topic: hyperion_protocol::SubscriptionTopic::Scene(
                                hyperion_protocol::SceneSubscribeRequest {
                                    detail: hyperion_protocol::DetailLevelDto::Contact,
                                    cameras: Vec::new(),
                                },
                            ),
                        },
                    ),
                )
                .await;
            let clock = SceneClockDto {
                time: UniverseTime {
                    seconds: 1,
                    nanos: 0,
                },
                time_rate: 1,
                state: SceneClockStateDto::Running,
            };
            let pusher = openings
                .next()
                .await
                .open(SubscriptionState::Scene(SceneStateDto {
                    sequence: 0,
                    clock,
                    ship: hyperion_protocol::KinematicsDto {
                        position: hyperion_protocol::FramePositionDto::Galactic {
                            position: hyperion_protocol::GalacticPosition::default(),
                        },
                        velocity_m_s: [0.0; 3],
                        time: UniverseTime::default(),
                    },
                    system: None,
                    tidal_radius_m: None,
                    craft: Vec::new(),
                }));
            assert!(matches!(
                client.next_frame().await,
                Received::Message(ServerMessage::Response { .. })
            ));
            client.request(2, body(2)).await;
            let bulk = BulkPayload::new(payload(FIELD_BYTES)).unwrap();
            assert!(calls.next().await.respond_bulk(small_response(), bulk));
            assert!(matches!(client.next_frame().await, Received::Binary(_)));
            assert!(pusher.push(PendingPush::Scene(ScenePush::heartbeat(clock))));
            let mut chunks_before = 1;
            loop {
                match client.next_frame().await {
                    Received::Binary(_) => chunks_before += 1,
                    Received::Message(ServerMessage::Notification { .. }) => break,
                    Received::Message(other) => panic!("the push came after {other:?}"),
                }
            }
            assert!(
                chunks_before < 58,
                "the push overtook the transfer's rest: {chunks_before}"
            );
            let (rest, terminal) = read_to_the_end(&mut client, 2).await;
            assert_eq!(chunks_before + rest.len(), 58);
            assert!(matches!(terminal, ServerMessage::Response { .. }));
            client.close().await;
            harness.stop().await;
        }

        #[tokio::test]
        async fn a_cancel_after_the_third_chunk_ends_the_transfer_with_cancelled() {
            let (handler, mut calls) = Scripted::new();
            let harness = Harness::start(handler).await;
            let mut client = harness.connect_slow_reader().await;
            client.hello().await;
            client.request(1, body(1)).await;
            let bulk = BulkPayload::new(payload(FIELD_BYTES)).unwrap();
            assert!(calls.next().await.respond_bulk(small_response(), bulk));
            for _ in 0..3 {
                assert!(matches!(client.next_frame().await, Received::Binary(_)));
            }
            client.cancel(1).await;
            // What was queued or in the socket before the cancel still arrives; nothing after.
            let (late, terminal) = read_to_the_end(&mut client, 1).await;
            match terminal {
                ServerMessage::RequestError { error, .. } => {
                    assert_eq!(error.code, ErrorCode::Cancelled);
                }
                other => panic!("expected `cancelled`, got {other:?}"),
            }
            assert!(
                3 + late.len() < 58,
                "the transfer stopped: {} late chunks",
                late.len()
            );
            client
                .send(&hyperion_protocol::ClientMessage::Ping { nonce: 9 })
                .await;
            match client.next_frame().await {
                Received::Message(ServerMessage::Pong { nonce: 9 }) => {}
                Received::Binary(_) => panic!("a chunk after `cancelled`"),
                Received::Message(other) => panic!("unexpected {other:?}"),
            }
            harness
                .requests_until(|counters| counters.in_flight() == 0)
                .await;
            client.close().await;
            harness.stop().await;
        }

        #[tokio::test]
        async fn a_stuck_reader_is_closed_by_the_write_timeout_with_nothing_held() {
            let (handler, mut calls) = Scripted::new();
            let harness = Harness::start_with_limits(
                handler,
                ConnectionLimits {
                    outbound_bytes: OUTBOUND_BYTES,
                    write_timeout: Duration::from_millis(500),
                    close_timeout: Duration::from_secs(1),
                },
            )
            .await;
            let mut client = harness.connect_slow_reader().await;
            client.hello().await;
            client.request(1, body(1)).await;
            let bulk = BulkPayload::new(payload(FIELD_BYTES)).unwrap();
            assert!(calls.next().await.respond_bulk(small_response(), bulk));
            // The client never reads.
            harness.connections_until(0).await;
            let outbound = harness
                .outbound_until(|counters| counters.write_timeouts() == 1)
                .await;
            assert_eq!((outbound.queued_bytes(), outbound.held_requests()), (0, 0));
            let requests = harness
                .requests_until(|counters| counters.in_flight() == 0)
                .await;
            assert_eq!(requests.abandoned(), 1);
            drop(client);
            harness.stop().await;
        }

        /// What a heartbeat push waits behind a 15 MB transfer over an emulated 40 Mbit/s link
        /// (Design note 11), with the low-water mark on and off: a reader with a fixed 64 KiB
        /// `SO_RCVBUF` reading through a 5 MB/s token bucket. Measured by hand, since a token
        /// bucket is real time; the figures go in the plan's notes, provisional on a shared
        /// machine.
        #[tokio::test]
        #[ignore = "measurement on an emulated slow link, run by hand: cargo test -p hyperion-server --lib bulk::tests::streaming::heartbeat_latency -- --ignored --nocapture"]
        async fn heartbeat_latency_behind_a_transfer_on_a_slow_link() {
            for tap in [Tap::LowWaterMark, Tap::Kernel] {
                let latencies = heartbeat_latencies(tap).await;
                let mut sorted = latencies.clone();
                sorted.sort_unstable();
                let median = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
                let worst = sorted.last().copied().unwrap_or_default();
                // A measurement's report, read by hand: there is no other channel for it.
                #[expect(
                    clippy::print_stderr,
                    reason = "a measurement run by hand reports its figures"
                )]
                {
                    eprintln!(
                        "{tap:?}: {} heartbeats during the transfer, median {median:?}, worst {worst:?}",
                        latencies.len()
                    );
                }
            }
        }

        /// The added latency of a push during a 15 MB transfer on loopback (rendering plan R03,
        /// R03.T15; Design note 11): ten craft pushed at 64 Hz, each push's delay from the instant
        /// its clock states to its receipt, for a second with no transfer and then during forty
        /// transfers read as fast as loopback carries them. Measured by hand, in real time; the
        /// figures go in the plan's notes, provisional on a shared machine.
        #[tokio::test]
        #[ignore = "measurement on loopback, run by hand: cargo test -p hyperion-server --lib bulk::tests::streaming::push_latency_on_loopback -- --ignored --nocapture"]
        #[expect(
            clippy::too_many_lines,
            reason = "a measurement run by hand, read top to bottom as one procedure"
        )]
        async fn push_latency_on_loopback() {
            use futures_util::{SinkExt, StreamExt};
            use tokio::time::Instant;
            use tokio_tungstenite::tungstenite::Message;

            let (handler, mut calls) = Scripted::new();
            let harness = Harness::start_tapped(
                SceneThenScripted(handler),
                ConnectionLimits::default(),
                |config| config.scene_knowledge(CraftSeen).craft_source(TenCraft),
                Tap::LowWaterMark,
            )
            .await;
            let state = std::sync::Arc::clone(harness.state());
            let universe = state
                .registry
                .create("Bulk".parse().unwrap(), Some(0x4d2))
                .await
                .unwrap();
            let start = hyperion_sim::time::UniverseTime::new(3_600, 0).unwrap();
            let anchor = Instant::now();
            let mut setting = state.scene.watch(universe.id()).borrow().clone();
            setting.clock = crate::scene::SceneClock::new(
                start,
                anchor,
                crate::scene::TimeRate::new(1).unwrap(),
            )
            .unwrap();
            state.scene.set(universe.id(), setting);

            let (mut socket, _) = tokio_tungstenite::connect_async(harness.url())
                .await
                .unwrap();
            let send = |message: &hyperion_protocol::ClientMessage| {
                Message::text(serde_json::to_string(message).unwrap())
            };
            socket
                .send(send(&hyperion_protocol::ClientMessage::Hello {
                    client_version: "measure".to_owned(),
                }))
                .await
                .unwrap();
            socket
                .send(send(&hyperion_protocol::ClientMessage::Request {
                    id: RequestId(1),
                    body: hyperion_protocol::RequestBody::Subscribe(
                        hyperion_protocol::SubscribeRequest {
                            universe: universe.id().into(),
                            topic: hyperion_protocol::SubscriptionTopic::Scene(
                                hyperion_protocol::SceneSubscribeRequest {
                                    detail: hyperion_protocol::DetailLevelDto::Contact,
                                    cameras: Vec::new(),
                                },
                            ),
                        },
                    ),
                }))
                .await
                .unwrap();
            // The delay of a push from the instant its clock states (1× from `start` at
            // `anchor`), or `None` for any other frame.
            let delay = |text: &str| match serde_json::from_str::<ServerMessage>(text) {
                Ok(ServerMessage::Notification {
                    body: hyperion_protocol::NotificationBody::Scene(notification),
                    ..
                }) => {
                    let at = hyperion_sim::time::UniverseTime::new(
                        notification.clock.time.seconds,
                        notification.clock.time.nanos,
                    )
                    .unwrap();
                    let since = at.checked_since(start).unwrap().as_seconds_f64();
                    Some(
                        Instant::now()
                            .saturating_duration_since(anchor + Duration::from_secs_f64(since)),
                    )
                }
                _ => None,
            };

            // The subscription live first: its opening builds the galaxy.
            while let Ok(Some(Ok(frame))) = tokio::time::timeout(WAIT, socket.next()).await {
                if let Message::Text(text) = frame
                    && let Ok(ServerMessage::Response {
                        id: RequestId(1), ..
                    }) = serde_json::from_str::<ServerMessage>(&text)
                {
                    break;
                }
            }
            // A second with no transfer.
            let mut idle = Vec::new();
            let quiet_until = Instant::now() + Duration::from_secs(1);
            while Instant::now() < quiet_until {
                let frame = tokio::time::timeout(WAIT, socket.next())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                if let Message::Text(text) = frame
                    && let Some(delay) = delay(&text)
                {
                    idle.push(delay);
                }
            }

            // Forty transfers, each push received between a transfer's first chunk and its last.
            let mut busy = Vec::new();
            let mut transfer_ms = Vec::new();
            for transfer in 0..40_u32 {
                let id = 2 + transfer;
                socket
                    .send(send(&hyperion_protocol::ClientMessage::Request {
                        id: RequestId(id),
                        body: body(id),
                    }))
                    .await
                    .unwrap();
                let bulk = BulkPayload::new(payload(FIELD_BYTES)).unwrap();
                assert!(calls.next().await.respond_bulk(small_response(), bulk));
                let mut chunks = 0;
                let mut first = None;
                while chunks < 58 {
                    let frame = tokio::time::timeout(WAIT, socket.next())
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap();
                    match frame {
                        Message::Binary(_) => {
                            chunks += 1;
                            first.get_or_insert_with(Instant::now);
                        }
                        Message::Text(text) => {
                            if let Some(delay) = delay(&text)
                                && chunks > 0
                            {
                                busy.push(delay);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(first) = first {
                    transfer_ms.push(first.elapsed().as_secs_f64() * 1e3);
                }
            }
            drop(socket);
            harness.stop().await;

            let summary = |mut delays: Vec<Duration>| {
                delays.sort_unstable();
                let median = delays.get(delays.len() / 2).copied().unwrap_or_default();
                let worst = delays.last().copied().unwrap_or_default();
                format!(
                    "{} pushes, median {median:?}, worst {worst:?}",
                    delays.len()
                )
            };
            // A measurement's report, read by hand: there is no other channel for it.
            #[expect(
                clippy::print_stderr,
                reason = "a measurement run by hand reports its figures"
            )]
            {
                eprintln!("idle: {}", summary(idle));
                eprintln!("during transfers: {}", summary(busy));
                eprintln!("transfers took {transfer_ms:.1?} ms from first chunk to last");
            }
        }

        /// Every craft is a contact; every body is granted what was asked.
        #[derive(Debug)]
        struct CraftSeen;

        impl crate::scene::SceneKnowledge for CraftSeen {
            fn grant(
                &self,
                _body: hyperion_sim::id::BodyId,
                asked: hyperion_sim::planetary::record::DetailLevel,
            ) -> hyperion_sim::planetary::record::DetailLevel {
                asked
            }

            fn is_contact(&self, _craft: &crate::scene::CraftState) -> bool {
                true
            }
        }

        /// Ten craft at rest at the galactic origin, each stating the scene time it was asked at.
        #[derive(Debug)]
        struct TenCraft;

        impl crate::scene::CraftSource for TenCraft {
            fn craft_at(
                &self,
                _universe: crate::universe::UniverseId,
                t: hyperion_sim::time::UniverseTime,
            ) -> Vec<crate::scene::CraftState> {
                (0..10_u8)
                    .map(|k| {
                        crate::scene::CraftState::new(hyperion_protocol::SceneCraftDto {
                            craft: format!("craft-{k}"),
                            hull: "test-hull".to_owned(),
                            state: hyperion_protocol::KinematicsDto {
                                position: hyperion_protocol::FramePositionDto::Galactic {
                                    position: hyperion_protocol::GalacticPosition::default(),
                                },
                                velocity_m_s: [f64::from(k), 0.0, 0.0],
                                time: UniverseTime {
                                    seconds: t.seconds(),
                                    nanos: t.subsec_nanos(),
                                },
                            },
                            attitude: [1.0, 0.0, 0.0, 0.0],
                            angular_velocity_rad_s: [0.0; 3],
                            planned_path: None,
                        })
                    })
                    .collect()
            }
        }

        /// The delay of each heartbeat received during one 15 MB transfer, from the instant the
        /// server's clock stated to the client's receipt.
        async fn heartbeat_latencies(tap: Tap) -> Vec<Duration> {
            use futures_util::SinkExt;
            use tokio::time::Instant;
            use tokio_tungstenite::tungstenite::Message;

            let (handler, mut calls) = Scripted::new();
            let harness = Harness::start_tapped(
                SceneThenScripted(handler),
                ConnectionLimits::default(),
                |config| config,
                tap,
            )
            .await;
            let state = std::sync::Arc::clone(harness.state());
            let universe = state
                .registry
                .create("Bulk".parse().unwrap(), Some(0x4d2))
                .await
                .unwrap();
            let start = hyperion_sim::time::UniverseTime::new(3_600, 0).unwrap();
            let anchor = Instant::now();
            let mut setting = state.scene.watch(universe.id()).borrow().clone();
            setting.clock = crate::scene::SceneClock::new(
                start,
                anchor,
                crate::scene::TimeRate::new(1).unwrap(),
            )
            .unwrap();
            state.scene.set(universe.id(), setting);

            let socket = tokio::net::TcpSocket::new_v4().unwrap();
            socket.set_recv_buffer_size(64 * 1024).unwrap();
            let stream = socket.connect(harness.addr()).await.unwrap();
            let (mut socket, _) = tokio_tungstenite::client_async(
                harness.url(),
                tokio_tungstenite::MaybeTlsStream::Plain(stream),
            )
            .await
            .unwrap();
            let send = |message: &hyperion_protocol::ClientMessage| {
                Message::text(serde_json::to_string(message).unwrap())
            };
            socket
                .send(send(&hyperion_protocol::ClientMessage::Hello {
                    client_version: "measure".to_owned(),
                }))
                .await
                .unwrap();
            socket
                .send(send(&hyperion_protocol::ClientMessage::Request {
                    id: RequestId(1),
                    body: hyperion_protocol::RequestBody::Subscribe(
                        hyperion_protocol::SubscribeRequest {
                            universe: universe.id().into(),
                            topic: hyperion_protocol::SubscriptionTopic::Scene(
                                hyperion_protocol::SceneSubscribeRequest {
                                    detail: hyperion_protocol::DetailLevelDto::Contact,
                                    cameras: Vec::new(),
                                },
                            ),
                        },
                    ),
                }))
                .await
                .unwrap();
            // The subscription live first: its opening builds the galaxy.
            while let Ok(Some(Ok(frame))) =
                tokio::time::timeout(WAIT, futures_util::StreamExt::next(&mut socket)).await
            {
                if let Message::Text(text) = frame
                    && let Ok(ServerMessage::Response {
                        id: RequestId(1), ..
                    }) = serde_json::from_str::<ServerMessage>(&text)
                {
                    break;
                }
            }
            socket
                .send(send(&hyperion_protocol::ClientMessage::Request {
                    id: RequestId(2),
                    body: body(2),
                }))
                .await
                .unwrap();
            let bulk = BulkPayload::new(payload(FIELD_BYTES)).unwrap();
            assert!(calls.next().await.respond_bulk(small_response(), bulk));

            let latencies = read_slowly(&mut socket, anchor, start).await;
            drop(socket);
            harness.stop().await;
            latencies
        }

        /// The emulated link's rate: 5 MB/s, 40 Mbit/s.
        const BYTES_PER_SECOND: f64 = 5.0e6;

        /// Reads the transfer through a token bucket of [`BYTES_PER_SECOND`], and returns the delay
        /// of each heartbeat that arrived during it, from the instant its clock states (the scene
        /// clock runs at 1× from `start` at `anchor`) to its receipt.
        async fn read_slowly(
            socket: &mut tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            anchor: tokio::time::Instant,
            start: hyperion_sim::time::UniverseTime,
        ) -> Vec<Duration> {
            use futures_util::StreamExt;
            use tokio::time::{Instant, sleep_until};
            use tokio_tungstenite::tungstenite::Message;

            // Each frame read waits until the bucket has had time to pass its bytes.
            let reading = Instant::now();
            let mut read = 0_usize;
            let mut chunks = 0;
            let mut latencies = Vec::new();
            while chunks < 58 {
                let frame = tokio::time::timeout(WAIT, socket.next())
                    .await
                    .expect("a frame within the wait")
                    .unwrap()
                    .unwrap();
                read += frame.len();
                let due = Duration::from_secs_f64(
                    f64::from(u32::try_from(read).unwrap()) / BYTES_PER_SECOND,
                );
                sleep_until(reading + due).await;
                match frame {
                    Message::Binary(_) => chunks += 1,
                    Message::Text(text) => {
                        if let Ok(ServerMessage::Notification {
                            body: hyperion_protocol::NotificationBody::Scene(notification),
                            ..
                        }) = serde_json::from_str::<ServerMessage>(&text)
                            && chunks > 0
                        {
                            let at = hyperion_sim::time::UniverseTime::new(
                                notification.clock.time.seconds,
                                notification.clock.time.nanos,
                            )
                            .unwrap();
                            let since = at.checked_since(start).unwrap().as_seconds_f64();
                            let stated_at = anchor + Duration::from_secs_f64(since);
                            latencies.push(Instant::now().saturating_duration_since(stated_at));
                        }
                    }
                    _ => {}
                }
            }
            latencies
        }

        /// Requests answered by a script, subscriptions by the server's own topics.
        #[derive(Debug)]
        struct SceneThenScripted(Scripted);

        impl crate::requests::Handler for SceneThenScripted {
            fn handle(
                &self,
                state: std::sync::Arc<crate::AppState>,
                body: hyperion_protocol::RequestBody,
                token: crate::compute::CancelToken,
            ) -> crate::requests::HandlerFuture {
                self.0.handle(state, body, token)
            }

            fn subscribe(
                &self,
                state: std::sync::Arc<crate::AppState>,
                request: hyperion_protocol::SubscribeRequest,
                pusher: crate::subscriptions::Pusher,
                token: crate::compute::CancelToken,
            ) -> crate::requests::SubscribeFuture {
                Handlers.subscribe(state, request, pusher, token)
            }
        }
    }
}
