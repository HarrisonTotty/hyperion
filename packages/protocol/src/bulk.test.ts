import { describe, expect, it } from "vitest";

import {
  BINARY_FRAME_HEADER_BYTES,
  BulkAssembler,
  MAX_BINARY_FRAME_BYTES,
  MAX_BULK_CHUNKS,
  MAX_BULK_PAYLOAD_BYTES,
  parseBinaryFrameHeader,
} from "./bulk";
import type { BulkManifestDto } from "./generated/BulkManifestDto";
import type { ClientMessage } from "./generated/ClientMessage";
import type { ResponseBody } from "./generated/ResponseBody";
import {
  isBulkAssemblyFailure,
  type PendingBulkRequest,
  RequestClient,
  type RequestFailure,
  setLastRequestId,
} from "./requests";

/** The header R03.T10.a pins in `crates/hyperion-server/src/bulk.rs`, byte for byte. */
const PINNED_HEADER = [
  [0x48, 0x59, 0x50, 0x42], // magic
  [1, 0], // format, reserved
  [24, 0], // header length
  [0x04, 0x03, 0x02, 0x01], // request
  [2, 0, 0, 0], // index
  [61, 0, 0, 0], // count
  [0xe8, 0xff, 0x03, 0x00], // payload length, 262,120
].flat();

/** A frame as the server's `encode_header` and `chunk` build it. */
function frame(
  request: number,
  index: number,
  count: number,
  payload: readonly number[],
): ArrayBuffer {
  const bytes = new ArrayBuffer(BINARY_FRAME_HEADER_BYTES + payload.length);
  const view = new DataView(bytes);
  [0x48, 0x59, 0x50, 0x42].forEach((byte, at) => {
    view.setUint8(at, byte);
  });
  view.setUint8(4, 1);
  view.setUint16(6, BINARY_FRAME_HEADER_BYTES, true);
  view.setUint32(8, request, true);
  view.setUint32(12, index, true);
  view.setUint32(16, count, true);
  view.setUint32(20, payload.length, true);
  new Uint8Array(bytes, BINARY_FRAME_HEADER_BYTES).set(payload);
  return bytes;
}

/** `frame` with one header byte changed. */
function withByte(bytes: ArrayBuffer, at: number, value: number): ArrayBuffer {
  const copy = bytes.slice(0);
  new DataView(copy).setUint8(at, value);
  return copy;
}

/** The response that ends a test's bulk request: any kind serves, since the manifest is read by the caller. */
const ANSWER: ResponseBody = {
  kind: "list_universes",
  universes: [],
  server_generator_version: 2,
};

/** A client whose `send` records every message, with its sent messages. */
function recordingClient(): { client: RequestClient; sent: ClientMessage[] } {
  const sent: ClientMessage[] = [];
  const client = new RequestClient((message) => {
    sent.push(message);
    return true;
  });
  return { client, sent };
}

/** Starts a bulk request whose response's manifest is `manifest`. */
function bulkRequest(client: RequestClient, manifest: BulkManifestDto | null) {
  return client.requestBulk({ kind: "list_universes" }, () => manifest);
}

/** The bytes of each chunk, as plain arrays. */
function bytesOf(chunks: ReadonlyArray<Uint8Array>): number[][] {
  return chunks.map((chunk) => [...chunk]);
}

describe("parseBinaryFrameHeader", () => {
  it("reads the header R03.T10.a pins", () => {
    const bytes = new ArrayBuffer(BINARY_FRAME_HEADER_BYTES + 262_120);
    new Uint8Array(bytes).set(PINNED_HEADER);

    const parsed = parseBinaryFrameHeader(bytes);

    expect(parsed).toMatchObject({
      ok: true,
      header: { request: 0x0102_0304, index: 2, count: 61, payloadBytes: 262_120 },
    });
    expect(bytes.byteLength).toBe(MAX_BINARY_FRAME_BYTES);
  });

  it("gives the payload as a view of the frame, not a copy", () => {
    const bytes = frame(7, 0, 1, [9, 8, 7]);

    const parsed = parseBinaryFrameHeader(bytes);

    if (!parsed.ok) {
      throw new Error(parsed.message);
    }
    expect(parsed.payload.buffer).toBe(bytes);
    expect([...parsed.payload]).toEqual([9, 8, 7]);
  });

  it.each([
    ["a bad magic", withByte(frame(7, 0, 1, [1]), 0, 0x58), /HYPB/],
    ["another format", withByte(frame(7, 0, 1, [1]), 4, 2), /format 2/],
    ["a reserved byte that is not 0", withByte(frame(7, 0, 1, [1]), 5, 1), /reserved byte is 1/],
    ["another header length", withByte(frame(7, 0, 1, [1]), 6, 25), /25 bytes/],
    ["a payload length the frame does not hold", withByte(frame(7, 0, 1, [1]), 20, 2), /2 payload/],
    ["an index outside its count", frame(7, 3, 3, [1]), /chunk 3 of 3/],
    ["a frame shorter than the header", new ArrayBuffer(10), /shorter/],
    ["a frame over 256 KiB", new ArrayBuffer(MAX_BINARY_FRAME_BYTES + 1), /over/],
  ])("refuses %s", (_, bytes, message) => {
    const parsed = parseBinaryFrameHeader(bytes);

    expect(parsed.ok).toBe(false);
    expect(parsed.ok ? "" : parsed.message).toMatch(message);
  });
});

describe("RequestClient with bulk answers", () => {
  it("resolves with the chunks in order once the manifest matches them", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 2, bytes: 5 });

    expect(client.handleBinaryFrame(frame(1, 0, 2, [1, 2, 3]))).toEqual({ ok: true });
    expect(client.handleBinaryFrame(frame(1, 1, 2, [4, 5]))).toEqual({ ok: true });
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    const outcome = await pending.outcome;
    if (!outcome.ok) {
      throw new Error(outcome.error.message);
    }
    expect(outcome.response).toEqual(ANSWER);
    expect(bytesOf(outcome.chunks)).toEqual([
      [1, 2, 3],
      [4, 5],
    ]);
  });

  it("hands over each partial answer whole, its chunks from 0 again, before the last", async () => {
    const { client } = recordingClient();
    const manifests: BulkManifestDto[] = [
      { chunks: 1, bytes: 2 },
      { chunks: 2, bytes: 3 },
      { chunks: 1, bytes: 1 },
    ];
    let read = 0;
    const partials: number[][][] = [];
    const pending = client.requestBulk(
      { kind: "list_universes" },
      () => {
        const manifest = manifests[read];
        read += 1;
        return manifest ?? null;
      },
      {
        onPartial: (answer) => {
          partials.push(bytesOf(answer.chunks));
        },
      },
    );

    client.handleBinaryFrame(frame(1, 0, 1, [1, 2]));
    client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });
    client.handleBinaryFrame(frame(1, 0, 2, [3, 4]));
    client.handleBinaryFrame(frame(1, 1, 2, [5]));
    client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });
    client.handleBinaryFrame(frame(1, 0, 1, [6]));
    expect(partials).toEqual([[[1, 2]], [[3, 4], [5]]]);
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    const outcome = await pending.outcome;
    if (!outcome.ok) {
      throw new Error(outcome.error.message);
    }
    expect(bytesOf(outcome.chunks)).toEqual([[6]]);
  });

  it("fails the request as internal and cancels it when a partial answer lacks its chunks", async () => {
    const { client, sent } = recordingClient();
    let partials = 0;
    const pending = client.requestBulk(
      { kind: "list_universes" },
      () => ({ chunks: 2, bytes: 4 }),
      {
        onPartial: () => {
          partials += 1;
        },
      },
    );

    client.handleBinaryFrame(frame(1, 0, 2, [1, 2]));
    client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/received 1 chunks/) },
    });
    expect(partials).toBe(0);
    expect(sent).toContainEqual({ type: "cancel", id: 1 });
  });

  it("fails a request answered in part with another kind as a protocol violation", async () => {
    const { client, sent } = recordingClient();
    const pending = client.requestBulk(
      { kind: "open_universe", universe: "000000000000002a" },
      () => null,
    );

    client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "protocol_violation" },
    });
    expect(sent).toContainEqual({ type: "cancel", id: 1 });
  });

  it("drops a partial answer's chunks for a request that takes no partial answers", async () => {
    const { client } = recordingClient();
    let read = 0;
    const pending = client.requestBulk({ kind: "list_universes" }, () => {
      read += 1;
      return read === 1 ? { chunks: 1, bytes: 2 } : { chunks: 1, bytes: 1 };
    });

    client.handleBinaryFrame(frame(1, 0, 1, [1, 2]));
    client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });
    client.handleBinaryFrame(frame(1, 0, 1, [3]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    const outcome = await pending.outcome;
    if (!outcome.ok) {
      throw new Error(outcome.error.message);
    }
    expect(bytesOf(outcome.chunks)).toEqual([[3]]);
  });

  it("drops a partial answer for a request that is not in flight", () => {
    const { client } = recordingClient();
    expect(client.handleServerMessage({ type: "partial_response", id: 9, body: ANSWER })).toBe(
      true,
    );
  });

  it("shows the per-chunk option every chunk in order before the outcome settles", async () => {
    const { client } = recordingClient();
    const seen: Array<{ bytes: number[]; index: number; count: number; settled: boolean }> = [];
    let settled = false;
    const pending = client.requestBulk(
      { kind: "list_universes" },
      () => ({ chunks: 3, bytes: 6 }),
      {
        onChunk: (chunk) => {
          seen.push({ bytes: [...chunk.payload], index: chunk.index, count: chunk.count, settled });
        },
      },
    );
    const settling = pending.outcome.then((outcome) => {
      settled = true;
      return outcome;
    });

    client.handleBinaryFrame(frame(1, 0, 3, [1, 2]));
    expect(seen).toHaveLength(1);
    client.handleBinaryFrame(frame(1, 1, 3, [3]));
    client.handleBinaryFrame(frame(1, 2, 3, [4, 5, 6]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    const outcome = await settling;
    expect(settled).toBe(true);
    expect(seen).toEqual([
      { bytes: [1, 2], index: 0, count: 3, settled: false },
      { bytes: [3], index: 1, count: 3, settled: false },
      { bytes: [4, 5, 6], index: 2, count: 3, settled: false },
    ]);
    // The chunks are still collected and handed over with the answer.
    expect(outcome.ok ? bytesOf(outcome.chunks) : outcome.error).toEqual([[1, 2], [3], [4, 5, 6]]);
  });

  it("fails the request as internal and cancels it when the per-chunk option throws", async () => {
    const { client, sent } = recordingClient();
    const pending = client.requestBulk(
      { kind: "list_universes" },
      () => ({ chunks: 2, bytes: 2 }),
      {
        onChunk: () => {
          throw new Error("no room for the block");
        },
      },
    );

    expect(client.handleBinaryFrame(frame(1, 0, 2, [1]))).toEqual({ ok: true });
    client.handleBinaryFrame(frame(1, 1, 2, [2]));

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/no room for the block/u) },
    });
    expect(sent).toContainEqual({ type: "cancel", id: 1 });
  });

  it("shows the per-chunk option no chunk that breaks its order, nor one after the request ended", async () => {
    const { client } = recordingClient();
    const seen: number[] = [];
    const pending = client.requestBulk(
      { kind: "list_universes" },
      () => ({ chunks: 3, bytes: 3 }),
      {
        onChunk: (chunk) => {
          seen.push(chunk.index);
        },
      },
    );

    client.handleBinaryFrame(frame(1, 0, 3, [1]));
    client.handleBinaryFrame(frame(1, 2, 3, [3]));
    client.handleBinaryFrame(frame(1, 1, 3, [2]));

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal" },
    });
    expect(seen).toEqual([0]);
  });

  it("shows the per-chunk option each partial answer's chunks, numbered from 0 for each", () => {
    const { client } = recordingClient();
    const seen: Array<readonly [number, number]> = [];
    client.requestBulk({ kind: "list_universes" }, () => ({ chunks: 1, bytes: 1 }), {
      onPartial: () => undefined,
      onChunk: (chunk) => {
        seen.push([chunk.index, chunk.payload[0] ?? -1]);
      },
    });

    client.handleBinaryFrame(frame(1, 0, 1, [7]));
    client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });
    client.handleBinaryFrame(frame(1, 0, 1, [8]));

    expect(seen).toEqual([
      [0, 7],
      [0, 8],
    ]);
  });

  it("resolves a response with no bulk and no chunks", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, null);

    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toEqual({ ok: true, response: ANSWER, chunks: [] });
  });

  it("fails the request as internal and cancels it when a chunk comes out of order", async () => {
    const { client, sent } = recordingClient();
    const pending = bulkRequest(client, { chunks: 3, bytes: 3 });

    client.handleBinaryFrame(frame(1, 0, 3, [1]));
    client.handleBinaryFrame(frame(1, 2, 3, [3]));

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/chunk 2 where chunk 1/) },
    });
    expect(sent).toContainEqual({ type: "cancel", id: 1 });
  });

  it("fails the request as internal when a chunk states another count", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 2, bytes: 2 });

    client.handleBinaryFrame(frame(1, 0, 2, [1]));
    client.handleBinaryFrame(frame(1, 1, 3, [2]));

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal" },
    });
  });

  it("fails the request as internal when a chunk is missing at the response", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 3, bytes: 3 });

    client.handleBinaryFrame(frame(1, 0, 3, [1]));
    client.handleBinaryFrame(frame(1, 1, 3, [2]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/2 chunks .* states 3/) },
    });
  });

  it("fails the request as internal when the bytes disagree with the manifest", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 1, bytes: 4 });

    client.handleBinaryFrame(frame(1, 0, 1, [1, 2, 3]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/3 bytes .* states 4/) },
    });
  });

  it("fails a response with no bulk that had chunks", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, null);

    client.handleBinaryFrame(frame(1, 0, 1, [1]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal" },
    });
  });

  it("discards the partial chunks of a cancelled request", async () => {
    const { client, sent } = recordingClient();
    const cancelled = bulkRequest(client, { chunks: 3, bytes: 3 });
    client.handleBinaryFrame(frame(1, 0, 3, [1]));
    client.handleBinaryFrame(frame(1, 1, 3, [2]));

    cancelled.cancel();
    // The server stops the chunks and ends the request in `cancelled`; a chunk already on its way
    // is dropped.
    client.handleBinaryFrame(frame(1, 2, 3, [3]));
    client.handleServerMessage({
      type: "request_error",
      id: 1,
      error: { code: "cancelled", message: "cancelled", field: null },
    });
    // The next request reuses the ID, and must see none of the cancelled one's chunks.
    client[setLastRequestId](0);
    const next = bulkRequest(client, { chunks: 1, bytes: 1 });
    client.handleBinaryFrame(frame(1, 0, 1, [9]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(cancelled.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "aborted" },
    });
    expect(sent).toContainEqual({ type: "cancel", id: 1 });
    const outcome = await next.outcome;
    expect(outcome.ok ? bytesOf(outcome.chunks) : outcome.error).toEqual([[9]]);
  });

  it("discards every request's chunks when the link drops", async () => {
    const { client } = recordingClient();
    const first = bulkRequest(client, { chunks: 2, bytes: 2 });
    const second = bulkRequest(client, { chunks: 2, bytes: 2 });
    client.handleBinaryFrame(frame(1, 0, 2, [1]));
    client.handleBinaryFrame(frame(2, 0, 2, [1]));

    client.linkLost();
    client[setLastRequestId](0);
    const next = bulkRequest(client, { chunks: 1, bytes: 1 });
    client.handleBinaryFrame(frame(1, 0, 1, [9]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(first.outcome).resolves.toMatchObject({ ok: false, error: { code: "link_lost" } });
    await expect(second.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "link_lost" },
    });
    const outcome = await next.outcome;
    expect(outcome.ok ? bytesOf(outcome.chunks) : outcome.error).toEqual([[9]]);
  });

  it("drops the chunks of a request that failed on the server", async () => {
    const { client } = recordingClient();
    const failed = bulkRequest(client, { chunks: 2, bytes: 2 });
    client.handleBinaryFrame(frame(1, 0, 2, [1]));

    client.handleServerMessage({
      type: "request_error",
      id: 1,
      error: { code: "internal", message: "the field failed", field: null },
    });

    client[setLastRequestId](0);
    const next = bulkRequest(client, { chunks: 1, bytes: 1 });
    client.handleBinaryFrame(frame(1, 0, 1, [9]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(failed.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: "the field failed" },
    });
    const outcome = await next.outcome;
    expect(outcome.ok ? bytesOf(outcome.chunks) : outcome.error).toEqual([[9]]);
  });

  it("fails the request when its chunks state more chunks than the manifest", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 2, bytes: 2 });

    client.handleBinaryFrame(frame(1, 0, 3, [1]));
    client.handleBinaryFrame(frame(1, 1, 3, [2]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/state 3 chunks .* states 2/) },
    });
  });

  it("fails the request, and settles, when its manifest cannot be read", async () => {
    const { client } = recordingClient();
    const pending = client.requestBulk({ kind: "list_universes" }, () => {
      throw new Error("no manifest");
    });

    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "internal", message: expect.stringMatching(/no manifest/) },
    });
  });

  it("drops a chunk for a request that asked for no bulk", async () => {
    const { client } = recordingClient();
    const plain = client.request({ kind: "list_universes" });

    expect(client.handleBinaryFrame(frame(1, 0, 1, [1]))).toEqual({ ok: true });
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    await expect(plain.outcome).resolves.toEqual({ ok: true, response: ANSWER });
  });

  it("reports a malformed frame without failing any request", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 1, bytes: 1 });

    const receipt = client.handleBinaryFrame(withByte(frame(1, 0, 1, [1]), 0, 0));
    client.handleBinaryFrame(frame(1, 0, 1, [1]));
    client.handleServerMessage({ type: "response", id: 1, body: ANSWER });

    expect(receipt).toMatchObject({ ok: false, message: expect.stringMatching(/HYPB/) });
    await expect(pending.outcome).resolves.toMatchObject({ ok: true });
  });
});

describe("BulkAssembler's limits", () => {
  const CHUNK_PAYLOAD = MAX_BINARY_FRAME_BYTES - BINARY_FRAME_HEADER_BYTES;

  it("are 64 MiB, in at most 257 chunks of the largest payload", () => {
    expect(MAX_BULK_PAYLOAD_BYTES).toBe(67_108_864);
    expect(MAX_BULK_CHUNKS).toBe(257);
  });

  it("fail a request whose chunk states more chunks than the limit", () => {
    const assembler = new BulkAssembler();
    assembler.expect(7);
    const header = { request: 7, index: 0, count: MAX_BULK_CHUNKS + 1, payloadBytes: 1 };

    expect(assembler.add(header, new Uint8Array(1))).toMatchObject({ ok: false, reason: "broken" });
    expect(assembler.add({ ...header, count: 1 }, new Uint8Array(1))).toEqual({
      ok: false,
      reason: "unexpected",
    });
  });

  it("fail a request at the chunk that carries it over the byte limit", () => {
    const assembler = new BulkAssembler();
    assembler.expect(7);
    const payload = new Uint8Array(CHUNK_PAYLOAD);
    const within = Math.floor(MAX_BULK_PAYLOAD_BYTES / CHUNK_PAYLOAD);
    const header = (index: number) => ({
      request: 7,
      index,
      count: MAX_BULK_CHUNKS,
      payloadBytes: CHUNK_PAYLOAD,
    });
    for (let index = 0; index < within; index += 1) {
      expect(assembler.add(header(index), payload)).toEqual({ ok: true });
    }

    expect(assembler.add(header(within), payload)).toMatchObject({ ok: false, reason: "broken" });
  });
});

/** The failure `pending` ended with. */
async function failureOf(pending: PendingBulkRequest<"list_universes">): Promise<RequestFailure> {
  const outcome = await pending.outcome;
  if (outcome.ok) {
    throw new Error("the request was answered");
  }
  return outcome.error;
}

describe("the client's own bulk failures (isBulkAssemblyFailure)", () => {
  it.each<[string, (client: RequestClient) => PendingBulkRequest<"list_universes">]>([
    [
      "a chunk out of order",
      (client) => {
        const pending = bulkRequest(client, { chunks: 3, bytes: 3 });
        client.handleBinaryFrame(frame(1, 0, 3, [1]));
        client.handleBinaryFrame(frame(1, 2, 3, [3]));
        return pending;
      },
    ],
    [
      "a chunk missing at the response",
      (client) => {
        const pending = bulkRequest(client, { chunks: 2, bytes: 2 });
        client.handleBinaryFrame(frame(1, 0, 2, [1]));
        client.handleServerMessage({ type: "response", id: 1, body: ANSWER });
        return pending;
      },
    ],
    [
      "a partial answer that lacks its chunks",
      (client) => {
        const pending = client.requestBulk(
          { kind: "list_universes" },
          () => ({ chunks: 2, bytes: 4 }),
          { onPartial: () => undefined },
        );
        client.handleBinaryFrame(frame(1, 0, 2, [1, 2]));
        client.handleServerMessage({ type: "partial_response", id: 1, body: ANSWER });
        return pending;
      },
    ],
    [
      "a per-chunk option that throws",
      (client) => {
        const pending = client.requestBulk(
          { kind: "list_universes" },
          () => ({ chunks: 1, bytes: 1 }),
          {
            onChunk: () => {
              throw new Error("no room for the block");
            },
          },
        );
        client.handleBinaryFrame(frame(1, 0, 1, [1]));
        return pending;
      },
    ],
    [
      "a manifest that cannot be read",
      (client) => {
        const pending = client.requestBulk({ kind: "list_universes" }, () => {
          throw new Error("no manifest");
        });
        client.handleServerMessage({ type: "response", id: 1, body: ANSWER });
        return pending;
      },
    ],
  ])("marks %s as the client's own, in the server's internal shape", async (_, start) => {
    const { client } = recordingClient();
    const error = await failureOf(start(client));
    expect([error, isBulkAssemblyFailure(error)]).toEqual([
      { code: "internal", message: expect.any(String), field: null, bulkUnassembled: true },
      true,
    ]);
  });

  it("does not mark the server's own internal failure", async () => {
    const { client } = recordingClient();
    const pending = bulkRequest(client, { chunks: 1, bytes: 1 });
    client.handleServerMessage({
      type: "request_error",
      id: 1,
      error: { code: "internal", message: "the field failed", field: null },
    });
    const error = await failureOf(pending);
    expect([error, isBulkAssemblyFailure(error)]).toEqual([
      { code: "internal", message: "the field failed", field: null },
      false,
    ]);
  });
});
