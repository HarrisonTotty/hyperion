/**
 * R03.T15's transfer check in the real renderer (plan R03's T15 "Pending"; R09.T19.c's record;
 * plan R10, R10.T15.b): one generated body's coarse field asked of a running server through
 * `RequestClient.requestBulk` over a real WebSocket, its chunks and its terminal response checked
 * whole and in order, as they arrived at the page's socket.
 *
 * @remarks
 * The harness's main process allows this one loopback WebSocket (`--smoke-transfer`), and the
 * runner, `scripts/transferCheck.sh`, starts the server and surveys the body before the page asks
 * (`just place-ship --survey`). Every unit test of the reassembly uses `FakeWebSocket`, so this is
 * the check over Chromium's own socket, frames and event loop. Nothing on its path uses the GPU.
 *
 * The page logs each message its socket takes, in order: each binary frame's header (its request,
 * index, count and length), and each text frame's terminal answer. {@link judgeTransfer} then
 * checks that the field was served, that every chunk came for the one request, in order, before
 * its terminal response, that the payload is whole against its manifest in frames of R03's size,
 * that it splits into R09's blocks ending at a block's end, and records its size against 15 MiB, 61
 * chunks: a body served at generator version 24 may not reach it (R09's header puts an Earth fully
 * surveyed at level 8 near 13.2 MB), and the check then records the payload it took.
 */

import {
  BINARY_FRAME_HEADER_BYTES,
  type BulkManifestDto,
  decodeServerMessage,
  encodeClientMessage,
  MAX_BINARY_FRAME_BYTES,
  parseBinaryFrameHeader,
  type RequestFailure,
  RequestClient,
  type ResponseFor,
} from "@hyperion/protocol";

import { BlockSplitter } from "../view/terrain/coarseFieldStore";
import type { Checks } from "./harness";

/** The payload R03's Design note 11 sizes the check by: 15 MiB, 15,728,640 bytes. */
export const TRANSFER_TARGET_BYTES = 15 * 1024 * 1024;

/** A full chunk's payload, bytes: a frame's most less its header, 262,120. */
export const CHUNK_PAYLOAD_BYTES = MAX_BINARY_FRAME_BYTES - BINARY_FRAME_HEADER_BYTES;

/** The chunks of a 15 MiB payload, ⌈15,728,640 ÷ 262,120⌉ = 61. */
export const TRANSFER_TARGET_CHUNKS = Math.ceil(TRANSFER_TARGET_BYTES / CHUNK_PAYLOAD_BYTES);

/** What the check asks for: the server's WebSocket, and the body's field in a universe. */
export interface TransferParams {
  readonly url: string;
  readonly universe: string;
  readonly body: string;
}

/** How long the check waits, ms. */
export interface TransferTiming {
  /** For the server's welcome: a page's first socket took 19.6 s once (R06.T11.b's record). */
  readonly welcomeMs: number;
  /**
   * For the field's answer: a level-8 coarse pass takes about a minute natively (R09.T16.a), and
   * longer on a slower machine, before its first chunk.
   */
  readonly answerMs: number;
}

/** The check's timing. */
export const TRANSFER_TIMING: TransferTiming = { welcomeMs: 60_000, answerMs: 540_000 };

/** One message the page's socket took, in the order it took them. */
export type TransferEvent =
  | {
      readonly kind: "chunk";
      readonly request: number;
      readonly index: number;
      readonly count: number;
      readonly payloadBytes: number;
      readonly frameBytes: number;
      readonly atMs: number;
    }
  /** A frame the client refused as malformed. */
  | { readonly kind: "malformed"; readonly message: string; readonly atMs: number }
  /** A text frame that ends a request: its response or its error. */
  | { readonly kind: "answer"; readonly request: number; readonly atMs: number };

/** What the transfer gave: the socket's log, the request's outcome, and the payload's reading. */
export interface TransferRun {
  readonly events: ReadonlyArray<TransferEvent>;
  /** The request's outcome, as `requestBulk` settled it. */
  readonly outcome:
    | {
        readonly ok: true;
        readonly response: ResponseFor<"surface_field">;
        readonly chunks: ReadonlyArray<Uint8Array>;
      }
    | { readonly ok: false; readonly error: RequestFailure };
  /** The server's generator version, from its welcome. */
  readonly generatorVersion: number;
  /** The blocks the payload split into, or why it could not be split. */
  readonly blocks:
    | { readonly ok: true; readonly count: number; readonly whole: boolean }
    | {
        readonly ok: false;
        readonly message: string;
      };
}

/** The 32-bit FNV-1a of `chunks` in order, eight lowercase hexadecimal digits. */
export function fnv1a32(chunks: ReadonlyArray<Uint8Array>): string {
  let hash = 0x811c9dc5;
  for (const chunk of chunks) {
    for (const byte of chunk) {
      hash = Math.imul(hash ^ byte, 0x01000193) >>> 0;
    }
  }
  return hash.toString(16).padStart(8, "0");
}

/** The figures of a transfer that arrived: its chunks, its bytes and its time, first chunk to answer. */
function transferFigures(
  chunks: ReadonlyArray<Extract<TransferEvent, { kind: "chunk" }>>,
  answer: Extract<TransferEvent, { kind: "answer" }> | undefined,
): string {
  const bytes = chunks.reduce((sum, chunk) => sum + chunk.payloadBytes, 0);
  const first = chunks[0];
  const spanMs = first === undefined || answer === undefined ? null : answer.atMs - first.atMs;
  return (
    `${bytes.toLocaleString("en-US")} bytes in ${String(chunks.length)} chunks` +
    (spanMs === null ? "" : `, ${spanMs.toFixed(0)} ms from the first chunk to the response`)
  );
}

/**
 * Checks a transfer ({@link TransferRun}) for the body asked: served, in order before its terminal
 * response, whole against its manifest in R03's frames, split into R09's blocks, and its size
 * recorded against 15 MiB (61 chunks), a check that always passes.
 *
 * @returns Whether the field was served; the later checks are made only where it was.
 */
export function judgeTransfer(params: TransferParams, run: TransferRun, checks: Checks): boolean {
  const { outcome } = run;
  const served = outcome.ok && outcome.response.state === "ready";
  let detail: string;
  if (!outcome.ok) {
    detail = `the request failed: ${outcome.error.code}: ${outcome.error.message}`;
  } else if (outcome.response.state === "not_modelled") {
    detail =
      `not_modelled: the server serves no surface for ${params.body} (its ` +
      `${outcome.response.section} section), at generator version ` +
      String(run.generatorVersion);
  } else {
    const { header, revision, coverage } = outcome.response;
    detail =
      `level ${String(header.level)}, revision ${String(revision)}, ` +
      `${String(coverage.ranges.length)} coverage ranges, at generator version ` +
      String(run.generatorVersion);
  }
  checks.check("R03.T15 the body's coarse field is served", served, detail);
  if (!outcome.ok || outcome.response.state !== "ready") {
    return false;
  }
  const manifest: BulkManifestDto = outcome.response.bulk;
  const answers = run.events.filter(
    (event): event is Extract<TransferEvent, { kind: "answer" }> => event.kind === "answer",
  );
  const answer = answers.at(-1);
  const request = answer?.request ?? null;
  const chunks = run.events.filter(
    (event): event is Extract<TransferEvent, { kind: "chunk" }> => event.kind === "chunk",
  );
  const answerAt = answer === undefined ? -1 : run.events.indexOf(answer);
  const malformed = run.events.filter((event) => event.kind === "malformed");
  const disorder = chunks.flatMap((chunk, i) => {
    const at = run.events.indexOf(chunk);
    const faults = [
      ...(chunk.request === request ? [] : [`request ${String(chunk.request)}`]),
      ...(chunk.index === i ? [] : [`index ${String(chunk.index)} in place ${String(i)}`]),
      ...(chunk.count === chunks.length ? [] : [`count ${String(chunk.count)}`]),
      ...(at < answerAt ? [] : ["after the response"]),
    ];
    return faults.length === 0 ? [] : [`chunk ${String(i)}: ${faults.join(", ")}`];
  });
  checks.check(
    "R03.T15 every chunk precedes the terminal response, in order",
    answer !== undefined && answers.length === 1 && disorder.length === 0 && malformed.length === 0,
    disorder.length > 0
      ? disorder.slice(0, 4).join("; ")
      : malformed.length > 0
        ? `${String(malformed.length)} malformed frames`
        : answers.length !== 1
          ? `${String(answers.length)} terminal answers`
          : `${String(chunks.length)} chunks of request ${String(request)}, then its response`,
  );
  const bytes = chunks.reduce((sum, chunk) => sum + chunk.payloadBytes, 0);
  const keptBytes = outcome.chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const oversized = chunks.filter((chunk) => chunk.frameBytes > MAX_BINARY_FRAME_BYTES).length;
  const short = chunks.filter(
    (chunk, i) => i < chunks.length - 1 && chunk.payloadBytes !== CHUNK_PAYLOAD_BYTES,
  ).length;
  checks.check(
    "R03.T15 the payload arrives whole, in R03's frames",
    manifest.chunks === chunks.length &&
      manifest.bytes === bytes &&
      keptBytes === bytes &&
      oversized === 0 &&
      short === 0,
    `manifest ${String(manifest.chunks)} chunks and ${String(manifest.bytes)} bytes, ` +
      `arrived ${String(chunks.length)} and ${String(bytes)} (${String(keptBytes)} kept), ` +
      `${String(oversized)} frames over ${String(MAX_BINARY_FRAME_BYTES)} bytes, ` +
      `${String(short)} short before the last; FNV-1a ${fnv1a32(outcome.chunks)}`,
  );
  const { blocks } = run;
  checks.check(
    "R03.T15 the payload splits into R09's blocks, ending at a block's end",
    blocks.ok && blocks.whole && blocks.count > 0,
    blocks.ok
      ? `${String(blocks.count)} blocks${blocks.whole ? "" : ", cut short within one"}`
      : blocks.message,
  );
  const reached = bytes >= TRANSFER_TARGET_BYTES && chunks.length >= TRANSFER_TARGET_CHUNKS;
  checks.check(
    `R03.T15 the transfer's size against 15 MiB (${String(TRANSFER_TARGET_CHUNKS)} chunks), recorded`,
    true,
    `${reached ? "reached" : "not reached, the largest payload this run served"}: ` +
      transferFigures(chunks, answer),
  );
  return true;
}

/** Rejects after `ms` with `what`, unless `promise` settles first. */
async function within<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${what} within ${String(ms / 1000)} s`));
    }, ms);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    clearTimeout(timer);
  }
}

/** Opens a WebSocket: the browser's, or a test's stand-in. */
export type OpenSocket = (url: string) => WebSocket;

/**
 * Runs the transfer: links to `params.url`, says hello, asks the body's field in bulk, logging
 * each message the socket takes, and closes the socket once the request settles.
 *
 * @throws Error when the socket fails or closes before the welcome, or no welcome or no answer
 *   comes within `timing`'s waits.
 */
export async function runTransfer(
  params: TransferParams,
  timing: TransferTiming = TRANSFER_TIMING,
  open: OpenSocket = (url) => new WebSocket(url),
  nowMs: () => number = () => performance.now(),
): Promise<TransferRun> {
  const socket = open(params.url);
  socket.binaryType = "arraybuffer";
  const events: TransferEvent[] = [];
  let welcomed = false;
  const client = new RequestClient((message) => {
    if (!welcomed) {
      return false;
    }
    socket.send(encodeClientMessage(message));
    return true;
  });
  let generatorVersion = 0;
  const welcome = new Promise<void>((resolve, reject) => {
    socket.addEventListener("open", () => {
      socket.send(encodeClientMessage({ type: "hello", client_version: "smoke-transfer" }));
    });
    socket.addEventListener("message", (event: MessageEvent<unknown>) => {
      const atMs = nowMs();
      if (event.data instanceof ArrayBuffer) {
        const parsed = parseBinaryFrameHeader(event.data);
        if (parsed.ok) {
          const { header } = parsed;
          events.push({
            kind: "chunk",
            request: header.request,
            index: header.index,
            count: header.count,
            payloadBytes: header.payloadBytes,
            frameBytes: event.data.byteLength,
            atMs,
          });
        }
        const receipt = client.handleBinaryFrame(event.data);
        if (!receipt.ok) {
          events.push({ kind: "malformed", message: receipt.message, atMs });
        }
        return;
      }
      if (typeof event.data !== "string") {
        return;
      }
      const message = decodeServerMessage(event.data);
      if (message.type === "response" || message.type === "request_error") {
        events.push({ kind: "answer", request: message.id, atMs });
      }
      if (client.handleServerMessage(message)) {
        return;
      }
      if (message.type === "welcome") {
        welcomed = true;
        generatorVersion = message.generator_version;
        resolve();
      } else if (message.type === "error") {
        console.error(`the server refused a message: ${message.message}`);
      }
    });
    socket.addEventListener("error", () => {
      reject(new Error(`the socket to ${params.url} failed`));
    });
    socket.addEventListener("close", () => {
      client.linkLost();
      reject(new Error(`the socket to ${params.url} closed`));
    });
  });
  try {
    await within(welcome, timing.welcomeMs, `no welcome from ${params.url}`);
    const splitter = new BlockSplitter();
    // Held in an object, which the chunks' callback updates as they come.
    const taking: { split: TransferRun["blocks"] } = {
      split: { ok: true, count: 0, whole: false },
    };
    const pending = client.requestBulk<"surface_field">(
      { kind: "surface_field", universe: params.universe, body: params.body, have_revision: null },
      (response) => (response.state === "ready" ? response.bulk : null),
      {
        onChunk: (chunk) => {
          const { split } = taking;
          if (!split.ok) {
            return;
          }
          const taken = splitter.push(chunk.payload);
          taking.split = taken.ok
            ? { ok: true, count: split.count + taken.blocks.length, whole: false }
            : { ok: false, message: taken.message };
        },
      },
    );
    const outcome = await within(pending.outcome, timing.answerMs, `no answer for ${params.body}`);
    const { split } = taking;
    const blocks: TransferRun["blocks"] = split.ok
      ? { ...split, whole: splitter.atBoundary }
      : split;
    return { events, outcome, generatorVersion, blocks };
  } finally {
    socket.close();
  }
}

/**
 * The transfer check: {@link runTransfer} then {@link judgeTransfer}, a failure to link or answer
 * recorded as the first check's.
 */
export async function checkTransfer(
  params: TransferParams,
  checks: Checks,
  timing: TransferTiming = TRANSFER_TIMING,
  open?: OpenSocket,
): Promise<void> {
  let run: TransferRun;
  try {
    run = await runTransfer(params, timing, open);
  } catch (error: unknown) {
    checks.check(
      "R03.T15 the body's coarse field is served",
      false,
      error instanceof Error ? error.message : String(error),
    );
    return;
  }
  judgeTransfer(params, run, checks);
}
