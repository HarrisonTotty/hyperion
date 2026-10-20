/**
 * `TERRAIN TEST`'s terrain fixture (plan R10, R10.T16.d; decision-r10-t16-contracts §4–5): the
 * request client a terrain session asks its fields of, answering `surface_field` from the
 * terrain golden's payloads as R09.T19.b answers them, and announcing the body's one revision rise
 * as the scene topic would.
 *
 * @remarks
 * - **A server in the page.** Each link is R03's own {@link RequestClient}, whose messages go to a
 *   stand-in of the server's `surface_field` handler here, which answers through the client's own
 *   entry points: each payload block as a binary frame of its own, then the answer, each on a
 *   task of its own, so that the chunks arrive asynchronously as a socket's do. The client's own
 *   rules hold then, as over the link: a cancel settles the request as `aborted` at once (and the
 *   stand-in sends nothing more for it), and the manifest is checked against the chunks.
 * - **The answers.** The whole field at the body's revision without `have_revision` (or with 0),
 *   the delta from the revision held below it, block 0 alone at the revision held, and
 *   `bad_request` naming `have_revision` above it, as R09.T19.b refuses one. A `body_detail`, or
 *   any other request, is `unsupported`: `TEST MOON` carries its own detail seed, so a session
 *   never asks it. Nothing reaches the server.
 * - **Revisions only rise.** A link starts at revision 1. The first time the script the stage
 *   plays has passed {@link TerrainTestFixtureOptions.deltaAtS} ({@link TerrainTestFixture.notePlayed},
 *   which `VIEW`'s loop calls each primary frame), the link's revision becomes 2 and the rise is
 *   announced once, as a scene notification's `surface_revisions` names it; whatever the script
 *   does after, a loop, a scrub back or a pause, it never falls. Each link (each session) starts
 *   afresh.
 * - **The field is read lazily**: the golden's blocks are decoded from their text at a link's
 *   first answer, once a fixture.
 * - **Its traffic** ({@link TerrainTestFixture.traffic}): the asks its links took and the blocks
 *   they sent and have still to send, over every link it opened, which the views check's terrain
 *   run reads around its forced device loss (R10.T16.d, part 2; decision-r10-t16-contracts §6).
 */
import {
  BINARY_FRAME_FORMAT,
  BINARY_FRAME_HEADER_BYTES,
  BINARY_FRAME_MAGIC,
  type ClientMessage,
  type RequestBody,
  type RequestError,
  RequestClient,
  type ResponseBody,
  type UniverseIdHex,
} from "@hyperion/protocol";

import type { SceneSurfaceEvent } from "../../lib/scene/useScene";
import type { TerrainFixture, TerrainFixtureLink } from "../../displays/view/useTerrainSession";
import { blockBytes, type TerrainGolden, type TerrainGoldenPayload } from "./terrainGolden";

/** What makes the terrain test's fixture. */
export interface TerrainTestFixtureOptions {
  /** The universe its links answer for, which no server holds. */
  readonly universe: UniverseIdHex;
  /** The field: the terrain golden as read ({@link TerrainGolden}). */
  readonly golden: () => TerrainGolden;
  /** The script time, s, at whose first passing a link's revision rises from 1 to 2. */
  readonly deltaAtS: number;
  /**
   * How long a link waits before each chunk and before the answer, ms: 0 by default (each on the
   * next task). The views check's terrain run paces its blocks with it, so that blocks are still
   * arriving when it forces a device loss (decision-r10-t16-contracts §6).
   */
  readonly chunkIntervalMs?: number;
}

/** What a fixture's links were asked and have sent, over every link it opened. */
export interface TerrainFixtureTraffic {
  /** Requests taken, of every kind. */
  readonly asks: number;
  /** Of those, `surface_field` asks. */
  readonly fieldAsks: number;
  /** Payload blocks sent, each as its binary frame. */
  readonly blocksSent: number;
  /** The blocks of the answers in flight still to send: asked, neither sent nor cancelled. */
  readonly blocksToCome: number;
}

/** The terrain test's fixture: a kept scene's {@link TerrainFixture} that follows its script. */
export interface TerrainTestFixture extends TerrainFixture {
  /**
   * Takes the time, s, the stage's script has played to (`ViewRun.tS`): every open link's
   * revision rises the first time it is at or past the delta's time.
   */
  readonly notePlayed: (scriptS: number) => void;
  /** Its links' traffic so far, a fresh copy each call. */
  readonly traffic: () => TerrainFixtureTraffic;
}

/** A fixture's traffic as its links count it. */
interface Traffic {
  asks: number;
  fieldAsks: number;
  blocksSent: number;
  blocksToCome: number;
}

/** The highest revision a link reaches: the delta's. */
const LAST_REVISION = 2;

/** One binary frame of a bulk answer, as the server's `encode_header` and `chunk` build it. */
function bulkFrame(
  request: number,
  index: number,
  count: number,
  payload: Uint8Array,
): ArrayBuffer {
  const bytes = new ArrayBuffer(BINARY_FRAME_HEADER_BYTES + payload.length);
  const view = new DataView(bytes);
  BINARY_FRAME_MAGIC.forEach((byte, at) => {
    view.setUint8(at, byte);
  });
  view.setUint8(4, BINARY_FRAME_FORMAT);
  view.setUint16(6, BINARY_FRAME_HEADER_BYTES, true);
  view.setUint32(8, request, true);
  view.setUint32(12, index, true);
  view.setUint32(16, count, true);
  view.setUint32(20, payload.length, true);
  new Uint8Array(bytes, BINARY_FRAME_HEADER_BYTES).set(payload);
  return bytes;
}

/** A refusal as the server words one. */
function refusal(code: RequestError["code"], field: string | null, message: string): RequestError {
  return { code, field, message };
}

/** The golden's blocks of each payload, decoded once a fixture at its first answer. */
class DecodedPayloads {
  readonly #golden: () => TerrainGolden;
  #payloads: ReadonlyArray<ReadonlyArray<Uint8Array>> | null = null;

  constructor(golden: () => TerrainGolden) {
    this.#golden = golden;
  }

  /** Payload `n`'s blocks (from 0, in the golden's order). */
  blocks(n: number): ReadonlyArray<Uint8Array> {
    this.#payloads ??= this.#golden().payloads.map((payload) => payload.blocksHex.map(blockBytes));
    const blocks = this.#payloads[n];
    if (blocks === undefined) {
      throw new Error(`the terrain golden has no payload ${String(n + 1)}`);
    }
    return blocks;
  }
}

/** One link: a request client over the stand-in of the server's handler. */
class FixtureLink implements TerrainFixtureLink {
  readonly requests: RequestClient;
  readonly #options: TerrainTestFixtureOptions;
  readonly #payloads: DecodedPayloads;
  readonly #listeners = new Set<(event: SceneSurfaceEvent) => void>();
  /** Each request's timers by its ID, until its answer is given or it is cancelled. */
  readonly #answering = new Map<number, Set<ReturnType<typeof setTimeout>>>();
  readonly #timers = new Set<ReturnType<typeof setTimeout>>();
  /** Each bulk answer's blocks still to send, by its request's ID. */
  readonly #toCome = new Map<number, number>();
  readonly #traffic: Traffic;
  readonly #onClose: () => void;
  #revision = 1;
  #closed = false;

  constructor(
    options: TerrainTestFixtureOptions,
    payloads: DecodedPayloads,
    traffic: Traffic,
    onClose: () => void,
  ) {
    this.#options = options;
    this.#payloads = payloads;
    this.#traffic = traffic;
    this.#onClose = onClose;
    this.requests = new RequestClient((message) => {
      this.#receive(message);
      return !this.#closed;
    });
  }

  /** The body's coverage revision as this link serves it: 1, then 2 once the script has passed the delta's time. */
  get revision(): number {
    return this.#revision;
  }

  readonly onSurface = (listener: (event: SceneSurfaceEvent) => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  readonly close = (): void => {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    for (const timer of this.#timers) {
      clearTimeout(timer);
    }
    this.#timers.clear();
    this.#answering.clear();
    for (const id of this.#toCome.keys()) {
      this.#forget(id);
    }
    this.#listeners.clear();
    this.requests.linkLost();
    this.#onClose();
  };

  /** The script has played to `scriptS`: the revision rises the first time it passes the delta's. */
  played(scriptS: number): void {
    if (this.#closed || this.#revision >= LAST_REVISION || !(scriptS >= this.#options.deltaAtS)) {
      return;
    }
    this.#revision = LAST_REVISION;
    const body = this.#options.golden().body;
    this.#later(() => {
      const event: SceneSurfaceEvent = {
        kind: "revisions",
        revisions: [{ body, revision: LAST_REVISION }],
      };
      for (const listener of this.#listeners) {
        listener(event);
      }
    }, 0);
  }

  /** Runs `then` on a task of its own after `delayMs`, unless the link closes first. */
  #later(then: () => void, delayMs: number, request?: number): void {
    const timer = setTimeout(() => {
      this.#timers.delete(timer);
      if (request !== undefined) {
        this.#answering.get(request)?.delete(timer);
      }
      then();
    }, delayMs);
    this.#timers.add(timer);
    if (request !== undefined) {
      this.#answering.get(request)?.add(timer);
    }
  }

  #receive(message: ClientMessage): void {
    if (this.#closed) {
      return;
    }
    switch (message.type) {
      case "request":
        this.#answer(message.id, message.body);
        break;
      case "cancel": {
        // Cancelled: the client has settled it as aborted, and nothing more is sent for it.
        for (const timer of this.#answering.get(message.id) ?? []) {
          clearTimeout(timer);
          this.#timers.delete(timer);
        }
        this.#answering.delete(message.id);
        this.#forget(message.id);
        break;
      }
      case "hello":
      case "ping":
        break;
    }
  }

  /** Request `id`'s blocks still to send are sent no more: it was cancelled, or its link closed. */
  #forget(id: number): void {
    this.#traffic.blocksToCome -= this.#toCome.get(id) ?? 0;
    this.#toCome.delete(id);
  }

  /** Answers request `id`, on later tasks. */
  #answer(id: number, body: RequestBody): void {
    this.#answering.set(id, new Set());
    this.#traffic.asks += 1;
    if (body.kind === "surface_field") {
      this.#traffic.fieldAsks += 1;
    }
    if (body.kind !== "surface_field") {
      this.#refuse(
        id,
        refusal(
          "unsupported",
          null,
          "the terrain test answers for its body's coarse field alone: the body carries its own detail seed",
        ),
      );
      return;
    }
    const golden = this.#options.golden();
    if (body.universe !== this.#options.universe) {
      this.#refuse(id, refusal("unknown_universe", "universe", `no universe ${body.universe}`));
      return;
    }
    if (body.body !== golden.body) {
      this.#refuse(
        id,
        refusal(
          "unknown_body",
          "body",
          `the terrain test serves the field of ${golden.body} alone`,
        ),
      );
      return;
    }
    const held = body.have_revision ?? 0;
    const revision = this.#revision;
    if (held > revision) {
      this.#refuse(
        id,
        refusal(
          "bad_request",
          "have_revision",
          `revision ${String(held)} is above the body's coverage revision, ${String(revision)}: ask again for the whole field`,
        ),
      );
      return;
    }
    const n = golden.payloads.findIndex((payload) => answers(payload, held, revision));
    const payload = golden.payloads[n];
    if (payload === undefined) {
      throw new Error(
        `the terrain golden has no payload at revision ${String(revision)} from ${String(held)}`,
      );
    }
    this.#send(id, payload, this.#payloads.blocks(n), golden, revision);
  }

  /** Sends payload `payload`'s blocks, one frame each, then its answer at `revision`. */
  #send(
    id: number,
    payload: TerrainGoldenPayload,
    blocks: ReadonlyArray<Uint8Array>,
    golden: TerrainGolden,
    revision: number,
  ): void {
    const pace = this.#options.chunkIntervalMs ?? 0;
    const count = blocks.length;
    const bytes = blocks.reduce((sum, block) => sum + block.length, 0);
    this.#toCome.set(id, count);
    this.#traffic.blocksToCome += count;
    const sendChunk = (index: number): void => {
      const block = blocks[index];
      if (block === undefined) {
        this.#toCome.delete(id);
        const answer: ResponseBody = {
          kind: "surface_field",
          state: "ready",
          header: golden.header,
          revision,
          coverage: payload.coverage,
          bulk: { chunks: count, bytes },
        };
        this.#answering.delete(id);
        this.requests.handleServerMessage({ type: "response", id, body: answer });
        return;
      }
      this.#toCome.set(id, count - index - 1);
      this.#traffic.blocksToCome -= 1;
      this.#traffic.blocksSent += 1;
      this.requests.handleBinaryFrame(bulkFrame(id, index, count, block));
      this.#later(
        () => {
          sendChunk(index + 1);
        },
        pace,
        id,
      );
    };
    this.#later(
      () => {
        sendChunk(0);
      },
      pace,
      id,
    );
  }

  /** Refuses request `id` on a later task. */
  #refuse(id: number, error: RequestError): void {
    this.#later(
      () => {
        this.#answering.delete(id);
        this.requests.handleServerMessage({ type: "request_error", id, error });
      },
      0,
      id,
    );
  }
}

/**
 * Whether `payload` is the answer to an ask from revision `held` (0 for none) at the body's
 * revision `revision`: the whole field, the delta from `held`, or, at the revision held, block 0
 * alone, which is the same at every revision (the golden's test holds it).
 */
function answers(payload: TerrainGoldenPayload, held: number, revision: number): boolean {
  if (held === revision) {
    return payload.coverage.ranges.length === 0;
  }
  return payload.revision === revision && (payload.have ?? 0) === held;
}

/**
 * The terrain test's fixture: links opened for each session, each a stand-in of the server's
 * `surface_field` over the golden's payloads, which follow the stage's script.
 */
export function terrainTestFixture(options: TerrainTestFixtureOptions): TerrainTestFixture {
  const payloads = new DecodedPayloads(options.golden);
  const open = new Set<FixtureLink>();
  const traffic: Traffic = { asks: 0, fieldAsks: 0, blocksSent: 0, blocksToCome: 0 };
  return {
    universe: options.universe,
    open: (): TerrainFixtureLink => {
      const link: FixtureLink = new FixtureLink(options, payloads, traffic, () => {
        open.delete(link);
      });
      open.add(link);
      return link;
    },
    notePlayed: (scriptS) => {
      for (const link of open) {
        link.played(scriptS);
      }
    },
    traffic: () => ({ ...traffic }),
  };
}
