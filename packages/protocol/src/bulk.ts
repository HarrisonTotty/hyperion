import type { BulkManifestDto } from "./generated/BulkManifestDto";
import type { RequestId } from "./generated/RequestId";

/** The first four bytes of every binary frame, `HYPB`. */
export const BINARY_FRAME_MAGIC: readonly [number, number, number, number] = [
  0x48, 0x59, 0x50, 0x42,
];

/** The binary frame header's format. */
export const BINARY_FRAME_FORMAT = 1;

/** The binary frame header's length in bytes. */
export const BINARY_FRAME_HEADER_BYTES = 24;

/**
 * The largest binary frame the server sends, 256 KiB with its header: the server's
 * `limits::MAX_BINARY_FRAME_BYTES` (rendering plan R03, Design note 11), restated here.
 */
export const MAX_BINARY_FRAME_BYTES = 262_144;

/**
 * One binary frame's header (rendering plan R03, Design note 10).
 *
 * @remarks
 * The 24 bytes are little-endian: the magic `HYPB`, the format 1, a reserved 0, the header's
 * length (24, `u16`), then as `u32`s the request's ID, the chunk's index from 0, the chunk count and
 * the payload's length in this frame. The layout is pinned by `crates/hyperion-server/src/bulk.rs`.
 */
export interface BinaryFrameHeader {
  /** The request the chunk answers. */
  readonly request: RequestId;
  /** The chunk's index, from 0. */
  readonly index: number;
  /** How many chunks the payload has. */
  readonly count: number;
  /** The payload bytes in this frame. */
  readonly payloadBytes: number;
}

/** A binary frame read: its header and a view of its payload, or why it was refused. */
export type ParsedBinaryFrame =
  | { readonly ok: true; readonly header: BinaryFrameHeader; readonly payload: Uint8Array }
  | { readonly ok: false; readonly message: string };

/**
 * Reads a binary frame's header and checks it against the frame.
 *
 * @remarks
 * Only the 24 header bytes are read; the payload is returned as a view of `frame`, not copied or
 * parsed, since its decoding is the requesting kind's affair (R06's sky, R09's coarse field), off
 * the main thread. A frame is refused for a short frame, a bad magic, format, reserved byte or
 * header length, a
 * payload length that disagrees with the frame's, a frame over {@link MAX_BINARY_FRAME_BYTES}, or
 * an index outside its count: each a server bug, which the link reports and survives.
 */
export function parseBinaryFrameHeader(frame: ArrayBuffer): ParsedBinaryFrame {
  if (frame.byteLength < BINARY_FRAME_HEADER_BYTES) {
    return refuse(`a binary frame of ${frame.byteLength} bytes is shorter than its header`);
  }
  if (frame.byteLength > MAX_BINARY_FRAME_BYTES) {
    return refuse(
      `a binary frame of ${frame.byteLength} bytes is over ${MAX_BINARY_FRAME_BYTES} bytes`,
    );
  }
  const view = new DataView(frame);
  if (BINARY_FRAME_MAGIC.some((byte, at) => view.getUint8(at) !== byte)) {
    return refuse("a binary frame does not start with HYPB");
  }
  const format = view.getUint8(4);
  if (format !== BINARY_FRAME_FORMAT) {
    return refuse(`a binary frame is of format ${format}, not ${BINARY_FRAME_FORMAT}`);
  }
  const reserved = view.getUint8(5);
  if (reserved !== 0) {
    return refuse(`a binary frame's reserved byte is ${reserved}, not 0`);
  }
  const headerBytes = view.getUint16(6, true);
  if (headerBytes !== BINARY_FRAME_HEADER_BYTES) {
    return refuse(
      `a binary frame's header states ${headerBytes} bytes, not ${BINARY_FRAME_HEADER_BYTES}`,
    );
  }
  const header: BinaryFrameHeader = {
    request: view.getUint32(8, true),
    index: view.getUint32(12, true),
    count: view.getUint32(16, true),
    payloadBytes: view.getUint32(20, true),
  };
  if (header.payloadBytes !== frame.byteLength - BINARY_FRAME_HEADER_BYTES) {
    return refuse(
      `a binary frame states ${header.payloadBytes} payload bytes and holds ` +
        `${frame.byteLength - BINARY_FRAME_HEADER_BYTES}`,
    );
  }
  if (header.index >= header.count) {
    return refuse(`a binary frame is chunk ${header.index} of ${header.count}`);
  }
  return {
    ok: true,
    header,
    payload: new Uint8Array(frame, BINARY_FRAME_HEADER_BYTES, header.payloadBytes),
  };
}

function refuse(message: string): ParsedBinaryFrame {
  return { ok: false, message };
}

/** What became of a chunk given to a {@link BulkAssembler}. */
export type ChunkReceipt =
  /** Kept for its request. */
  | { readonly ok: true }
  /** No request is waiting for chunks under its ID: it has ended, or never asked for bulk. */
  | { readonly ok: false; readonly reason: "unexpected" }
  /** Out of order, or disagreeing with the request's earlier chunks: the request has failed. */
  | { readonly ok: false; readonly reason: "broken"; readonly message: string };

/** A bulk payload as it arrived, checked against its manifest, or why it does not match. */
export type AssembledBulk =
  | { readonly ok: true; readonly chunks: ReadonlyArray<Uint8Array> }
  | { readonly ok: false; readonly message: string };

/** The chunks of one request so far. */
interface Collecting {
  readonly chunks: Uint8Array[];
  /** The chunk count the first chunk stated; `null` until a chunk has arrived. */
  count: number | null;
  bytes: number;
}

/**
 * Collects the binary chunks of the requests answered in bulk, by request ID, until each one's
 * terminal response.
 *
 * @remarks
 * The chunks come in order before their request's terminal response (Design note 10), so a chunk
 * whose index is not the next expected is a missing or reordered one, and fails its request at
 * once. Each chunk is kept as the view {@link parseBinaryFrameHeader} gave, over the frame's own
 * `ArrayBuffer`, never copied or parsed (Design note 11).
 */
export class BulkAssembler {
  readonly #collecting = new Map<RequestId, Collecting>();

  /** Waits for chunks answering `request`. */
  expect(request: RequestId): void {
    this.#collecting.set(request, { chunks: [], count: null, bytes: 0 });
  }

  /**
   * Keeps one chunk for its request.
   *
   * @remarks
   * A broken chunk discards everything its request had; the caller fails the request.
   */
  add(header: BinaryFrameHeader, payload: Uint8Array): ChunkReceipt {
    const collecting = this.#collecting.get(header.request);
    if (collecting === undefined) {
      return { ok: false, reason: "unexpected" };
    }
    const broken = (message: string): ChunkReceipt => {
      this.#collecting.delete(header.request);
      return { ok: false, reason: "broken", message };
    };
    if (collecting.count !== null && header.count !== collecting.count) {
      return broken(
        `chunk ${header.index} of request ${header.request} states ${header.count} chunks, ` +
          `after ${collecting.count}`,
      );
    }
    if (header.index !== collecting.chunks.length) {
      return broken(
        `request ${header.request} received chunk ${header.index} where chunk ` +
          `${collecting.chunks.length} was due`,
      );
    }
    collecting.count = header.count;
    collecting.chunks.push(payload);
    collecting.bytes += payload.byteLength;
    return { ok: true };
  }

  /**
   * Ends collecting for `request` at its terminal response, and checks what arrived against the
   * response's manifest.
   *
   * @param manifest - The response's manifest, or `null` for a response that states no bulk, which
   *   matches no chunks at all.
   */
  finish(request: RequestId, manifest: BulkManifestDto | null): AssembledBulk {
    const collecting = this.#collecting.get(request) ?? { chunks: [], count: null, bytes: 0 };
    this.#collecting.delete(request);
    const expected = manifest ?? { chunks: 0, bytes: 0 };
    if (collecting.chunks.length !== expected.chunks) {
      return {
        ok: false,
        message:
          `request ${request} received ${collecting.chunks.length} chunks and its response ` +
          `states ${expected.chunks}`,
      };
    }
    if (collecting.count !== null && collecting.count !== expected.chunks) {
      return {
        ok: false,
        message:
          `request ${request}'s chunks state ${collecting.count} chunks and its response ` +
          `states ${expected.chunks}`,
      };
    }
    if (collecting.bytes !== expected.bytes) {
      return {
        ok: false,
        message:
          `request ${request} received ${collecting.bytes} bytes and its response states ` +
          `${expected.bytes}`,
      };
    }
    return { ok: true, chunks: collecting.chunks };
  }

  /** Drops whatever `request` had, for a request that failed or was cancelled. */
  discard(request: RequestId): void {
    this.#collecting.delete(request);
  }

  /** Drops every request's chunks, for when the link drops. */
  clear(): void {
    this.#collecting.clear();
  }
}
