import type { BodyIdHex, ResponseFor } from "@hyperion/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  fieldBlock,
  fieldCover,
  fieldPayload,
  latestFieldAsk,
  moonLikeHeader,
  readyField,
  sendFieldFrames,
} from "../test/coarseFieldFixtures";
import { FakeWebSocket } from "../test/FakeWebSocket";
import { Checks } from "./harness";
import {
  CHUNK_PAYLOAD_BYTES,
  checkTransfer,
  fnv1a32,
  judgeTransfer,
  TRANSFER_TARGET_BYTES,
  TRANSFER_TARGET_CHUNKS,
  type TransferEvent,
  type TransferParams,
  type TransferRun,
} from "./transfer";

const BODY: BodyIdHex = "00000000000000aa.0100";
const PARAMS: TransferParams = {
  url: "ws://127.0.0.1:7881/ws",
  universe: "0000000000000001",
  body: BODY,
};
const TIMING = { welcomeMs: 1_000, answerMs: 1_000 };

/** Lets the page take what the fake socket dispatched. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });
}

/** A `ready` answer whose payload is `blocks`, sent in R03's full chunks. */
function servePayload(socket: FakeWebSocket, blocks: ReadonlyArray<Uint8Array>): string {
  const { id } = latestFieldAsk(socket);
  const payload = fieldPayload(blocks);
  const bulk = sendFieldFrames(socket, id, payload, CHUNK_PAYLOAD_BYTES);
  socket.serverResponds(id, readyField(moonLikeHeader(BODY), 1, fieldCover([0, 6_144, 9]), bulk));
  return fnv1a32([payload]);
}

/** The checks of one transfer over the fake socket, the server played by `serve`. */
async function transferChecks(
  serve: (socket: FakeWebSocket) => void,
): Promise<
  ReadonlyArray<{ readonly name: string; readonly pass: boolean; readonly detail: string }>
> {
  const checks = new Checks();
  const done = checkTransfer(PARAMS, checks, TIMING);
  const socket = FakeWebSocket.latest();
  socket.serverWelcomes(24);
  await settle();
  serve(socket);
  await done;
  return checks.list;
}

/** A transfer's run of `chunks` chunks of request 7 before its answer, each `bytes` long. */
function runOf(
  sizes: ReadonlyArray<number>,
  order: ReadonlyArray<number> = sizes.map((_, i) => i),
  answerAt: number = sizes.length,
): TransferRun {
  const chunkEvents: TransferEvent[] = order.map((index) => ({
    kind: "chunk",
    request: 7,
    index,
    count: sizes.length,
    payloadBytes: sizes[index] ?? 0,
    frameBytes: (sizes[index] ?? 0) + 24,
    atMs: index,
  }));
  const answer: TransferEvent = { kind: "answer", request: 7, atMs: 100 };
  const events = [...chunkEvents.slice(0, answerAt), answer, ...chunkEvents.slice(answerAt)];
  const bytes = sizes.reduce((sum, size) => sum + size, 0);
  const response: ResponseFor<"surface_field"> = readyField(
    moonLikeHeader(BODY),
    1,
    fieldCover([0, 6_144, 9]),
    { chunks: sizes.length, bytes },
  );
  return {
    events,
    outcome: { ok: true, response, chunks: sizes.map((size) => new Uint8Array(size)) },
    generatorVersion: 24,
    blocks: { ok: true, count: 3, whole: true },
  };
}

function judged(
  run: TransferRun,
): ReadonlyArray<{ readonly name: string; readonly pass: boolean }> {
  const checks = new Checks();
  judgeTransfer(PARAMS, run, checks);
  return checks.list;
}

beforeEach(() => {
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  FakeWebSocket.instances = [];
  vi.useRealTimers();
});

describe("R03.T15's transfer check over a socket (R10.T15.b)", () => {
  it("passes a field whose chunks come whole and in order before its response", async () => {
    let hash = "";
    const checks = await transferChecks((socket) => {
      // Three blocks over three chunks, the last short.
      hash = servePayload(socket, [
        fieldBlock(200_000, 1),
        fieldBlock(300_000, 2),
        fieldBlock(100_000, 3),
      ]);
    });
    expect(checks.map(({ name, pass }) => [name, pass])).toEqual([
      ["R03.T15 the body's coarse field is served", true],
      ["R03.T15 every chunk precedes the terminal response, in order", true],
      ["R03.T15 the payload arrives whole, in R03's frames", true],
      ["R03.T15 the payload splits into R09's blocks, ending at a block's end", true],
      ["R03.T15 the transfer's size against 15 MiB (61 chunks), recorded", true],
    ]);
    expect(checks[2]?.detail).toContain(`FNV-1a ${hash}`);
    expect(checks[3]?.detail).toBe("3 blocks");
    expect(checks[4]?.detail).toMatch(
      /^not reached, the largest payload this run served: 600,000 bytes in 3 chunks/u,
    );
  });

  it("asks the body's whole field in bulk, saying hello first", async () => {
    await transferChecks((socket) => {
      expect(socket.sent[0]).toEqual({ type: "hello", client_version: "smoke-transfer" });
      expect(socket.requestsOfKind("surface_field").map(({ body }) => body)).toEqual([
        { kind: "surface_field", universe: PARAMS.universe, body: BODY, have_revision: null },
      ]);
      servePayload(socket, [fieldBlock(100, 1)]);
    });
  });

  it("says plainly that the body has no served surface when it is not modelled", async () => {
    const checks = await transferChecks((socket) => {
      const { id } = latestFieldAsk(socket);
      socket.serverResponds(id, {
        kind: "surface_field",
        state: "not_modelled",
        section: "surface",
      });
    });
    expect(checks).toEqual([
      {
        name: "R03.T15 the body's coarse field is served",
        pass: false,
        detail: `not_modelled: the server serves no surface for ${BODY} (its surface section), at generator version 24`,
      },
    ]);
  });

  it("fails as unserved when the server never welcomes the page", async () => {
    vi.useFakeTimers();
    const checks = new Checks();
    const done = checkTransfer(PARAMS, checks, TIMING);
    FakeWebSocket.latest().serverOpens();
    await vi.advanceTimersByTimeAsync(TIMING.welcomeMs);
    await done;
    expect(checks.list).toEqual([
      {
        name: "R03.T15 the body's coarse field is served",
        pass: false,
        detail: `no welcome from ${PARAMS.url} within 1 s`,
      },
    ]);
  });
});

describe("the transfer's judgement", () => {
  const full = CHUNK_PAYLOAD_BYTES;

  it("records a 15 MiB transfer in 61 chunks as reached", () => {
    const sizes = [
      ...Array.from({ length: TRANSFER_TARGET_CHUNKS - 1 }, () => full),
      TRANSFER_TARGET_BYTES - (TRANSFER_TARGET_CHUNKS - 1) * full,
    ];
    const checks = new Checks();
    expect(judgeTransfer(PARAMS, runOf(sizes), checks)).toBe(true);
    expect(checks.list.every((check) => check.pass)).toBe(true);
    expect(checks.list.at(-1)?.detail).toMatch(/^reached: 15,728,640 bytes in 61 chunks/u);
    expect([TRANSFER_TARGET_BYTES, TRANSFER_TARGET_CHUNKS]).toEqual([15_728_640, 61]);
  });

  it("fails a chunk out of order", () => {
    const checks = judged(runOf([full, full, 10], [0, 2, 1]));
    expect(checks.find((check) => check.name.includes("in order"))?.pass).toBe(false);
  });

  it("fails a chunk after the terminal response", () => {
    const checks = judged(runOf([full, full, 10], [0, 1, 2], 2));
    expect(checks.find((check) => check.name.includes("in order"))?.pass).toBe(false);
  });

  it("fails a short chunk before the last, and a manifest unlike what came", () => {
    expect(judged(runOf([full, 10, 10]))[2]?.pass).toBe(false);
    const run = runOf([full, 10]);
    const outcome = run.outcome;
    if (!outcome.ok || outcome.response.state !== "ready") {
      throw new Error("the run is served");
    }
    const unlike: TransferRun = {
      ...run,
      outcome: { ...outcome, response: { ...outcome.response, bulk: { chunks: 2, bytes: 1 } } },
    };
    expect(judged(unlike)[2]?.pass).toBe(false);
  });

  it("fails a payload cut within a block", () => {
    const run: TransferRun = { ...runOf([full, 10]), blocks: { ok: true, count: 1, whole: false } };
    expect(judged(run)[3]).toMatchObject({ pass: false });
  });
});

describe("the payload's hash", () => {
  it("is FNV-1a's 32 bits, over the chunks in order", () => {
    const a = new TextEncoder().encode("a");
    const foobar = new TextEncoder().encode("foobar");
    expect(fnv1a32([])).toBe("811c9dc5");
    expect(fnv1a32([a])).toBe("e40c292c");
    expect(fnv1a32([foobar])).toBe("bf9cf968");
    expect(fnv1a32([foobar.subarray(0, 2), foobar.subarray(2)])).toBe("bf9cf968");
  });
});
