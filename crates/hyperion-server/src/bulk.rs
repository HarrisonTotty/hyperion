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
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R03.T10.b streams the frames; until then only the tests cut them"
    )
)]

use std::error::Error;
use std::fmt;

use axum::body::Bytes;
use hyperion_protocol::{BulkManifestDto, RequestId, ResponseBody};

use crate::limits::MAX_BINARY_FRAME_BYTES;

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
}
