import type {
  BodyIdHex,
  DetailSeedHex,
  ResponseFor,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { instrumentViewId, PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { vec3 } from "../../geometry/vec3";
import { type KeplerOrbit, meanAnomalyAt } from "../../lib/orbit";
import { toKeplerOrbit } from "../../lib/system/wire";
import { type ServerLink, useServerLink } from "../../lib/serverLink";
import {
  answerBlock,
  blockChange,
  fieldBlock,
  fieldCover,
  fieldPayload,
  latestFieldAsk,
  moonLikeHeader,
  readyField,
  sendFieldFrames,
} from "../../test/coarseFieldFixtures";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { earthDetail } from "../../test/planetaryFixture";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  moonRangeM,
  SESSION_MOON,
  SESSION_MOON_RADIUS_M,
  SESSION_PLANET,
  SESSION_SCENE,
  viewAt,
} from "../../test/terrainSessionFixtures";
import { aBody, FIXTURE_PLANET_CENTRE_M, FIXTURE_SYSTEM } from "../../test/viewFixtures";
import { lawFor } from "../appearance/phase";
import type { AllocationEvent } from "../engine/memory";
import type { ViewBody, ViewScene } from "../scene/model";
import { TERRAIN_COMPUTATION_FAILED, TERRAIN_NOT_AVAILABLE } from "./annunciation";
import { CLASS_MAP_LEVEL } from "./classMap";
import { fieldCellAt } from "./coverage";
import { allocationTally } from "./gpu/allocationTally";
import { limbDistanceM } from "./handover";
import { TerrainSession, viewFieldCandidates } from "./session";
import { PHASE_LAW } from "./terrainMaterial";
import type { ClassMapAnswer, GroundReadoutRequest } from "./workers/messages";
import { MAX_CONSECUTIVE_WORKER_FAILURES } from "./workers/pool";

const UNIVERSE: UniverseIdHex = "000000000000002a";
const MOON: BodyIdHex = SESSION_MOON;
const PLANET: BodyIdHex = SESSION_PLANET;
const SCENE: ViewScene = SESSION_SCENE;
const MOON_RADIUS_M = SESSION_MOON_RADIUS_M;
const SEED: DetailSeedHex = "0123456789abcdef";
const PRIMARY = PRIMARY_VIEW_ID;
const INSTRUMENT = instrumentViewId(1);

/** Resolves after `hops` turns of the microtask queue. */
function microtasks(hops: number): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(() => microtasks(hops - 1));
}

/** Lets the outcomes' promise chains settle. */
async function settle(): Promise<void> {
  await act(async () => {
    await microtasks(10);
  });
}

interface Harness {
  readonly socket: () => FakeWebSocket;
  readonly link: () => ServerLink;
  readonly engine: CountingRenderEngine;
  readonly workers: ScriptedHeightWorker[];
  readonly session: TerrainSession;
  /** Every allocation event since before the session was made. */
  readonly allocations: AllocationEvent[];
  readonly tally: ReturnType<typeof allocationTally>;
}

async function harness(options: { readonly workers?: number | "default" } = {}): Promise<Harness> {
  const hook = renderHook(() => useServerLink(), { wrapper: ServerLinkHarness });
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  const engine = await countingRenderEngine();
  const allocations: AllocationEvent[] = [];
  engine.onAllocation((event) => allocations.push(event));
  const tally = allocationTally(engine);
  const workers: ScriptedHeightWorker[] = [];
  const session = new TerrainSession({
    engine,
    universe: UNIVERSE,
    requests: hook.result.current.requests,
    linkUp: true,
    setting: "high",
    createWorker: () => {
      const worker = new ScriptedHeightWorker();
      workers.push(worker);
      return worker;
    },
    // Two workers unless a test asks for Design note 11's default count.
    ...(options.workers === "default" ? {} : { workers: options.workers ?? 2 }),
  });
  return {
    socket: () => FakeWebSocket.latest(),
    link: () => hook.result.current,
    engine,
    workers,
    session,
    allocations,
    tally,
  };
}

/** `workers[n]`, which must exist. */
function workerAt(h: Harness, n: number): ScriptedHeightWorker {
  const worker = h.workers[n];
  if (worker === undefined) {
    throw new Error(`no worker ${String(n)}`);
  }
  return worker;
}

/** The latest field request's blocks sent, before its answer: one block of 50 bytes by default. */
function sendBlocks(h: Harness, blocks = [fieldBlock(50, 1)]): ReturnType<typeof sendFieldFrames> {
  return sendFieldFrames(h.socket(), latestFieldAsk(h.socket()).id, fieldPayload(blocks));
}

/** Answers the latest field request with one block, at `revision`, cells 0 to 2 surveyed. */
async function answerField(h: Harness, revision = 1, body: BodyIdHex = MOON): Promise<void> {
  const { id } = latestFieldAsk(h.socket());
  const bulk = sendBlocks(h);
  h.socket().serverResponds(
    id,
    readyField(moonLikeHeader(body), revision, fieldCover([0, 2, 30]), bulk),
  );
  await settle();
}

/** The fixture's Earth's record answered, its hooks `ok` with `seed`, or not modelled for `null`. */
function detailWithSeed(seed: DetailSeedHex | null): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  return {
    ...detail,
    record: {
      ...detail.record,
      hooks:
        seed === null
          ? { state: "not_modelled" }
          : { state: "ok", value: { detail_seed: { state: "ok", value: seed } } },
    },
  };
}

/** A class map of one class over one texel, as a worker answers it. */
function oneTexelMap(): ClassMapAnswer {
  return {
    level: CLASS_MAP_LEVEL,
    fieldLevel: 5,
    classes: Uint8Array.of(1, 0),
    photometry: Float32Array.of(0.1, 0.11, 0.12, PHASE_LAW.regolith, 1),
    texels: Uint32Array.of(0),
    fractions: Float32Array.of(1),
    weights: Uint8Array.of(255),
    complete: true,
  };
}

/** The moon wanted as the primary's local body, its field answered at revision 1. */
async function holdMoon(h: Harness): Promise<void> {
  h.session.noteView(PRIMARY, viewAt(5e6));
  h.session.step();
  await answerField(h);
}

/** The texel of the coverage mask that holds cell `index` of level `level`. */
function maskTexelOf(level: number, index: number): number {
  const n = 2 ** level;
  const cell = fieldCellAt(level, index);
  return (cell.face * n + cell.j) * n + cell.i;
}

/** The latest bytes written to `body`'s coverage mask. */
function maskWritten(h: Harness, body: BodyIdHex): Uint8Array | undefined {
  return h.engine.textureWritten.findLast((write) => write.texture === `coverage mask ${body}`)
    ?.data;
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("a view's field candidates (R10.T16.a)", () => {
  it("marks the body whose frame the camera is in as its local body", () => {
    const local = viewFieldCandidates(viewAt(5e6), () => null).map((c) => [c.body, c.local]);
    expect(local).toEqual([
      [PLANET, false],
      [MOON, true],
    ]);
  });

  it("gives a body its limb's distance over its range at the style's τ", () => {
    const [, moon] = viewFieldCandidates(viewAt(2e7, "wireframe"), () => 18_000);
    expect(moon?.rangeRatio).toBeCloseTo(limbDistanceM(2e7, MOON_RADIUS_M) / moonRangeM(4), 12);
  });

  it("gives a body with no relief known an infinite ratio", () => {
    const [, moon] = viewFieldCandidates(viewAt(2e7), () => null);
    expect(moon?.rangeRatio).toBe(Number.POSITIVE_INFINITY);
  });

  it("marks the body whose terrain readout the view states as a local body too (R10.T12)", () => {
    const report = { ...viewAt(5e6), readoutBody: PLANET };
    const local = viewFieldCandidates(report, () => null).map((c) => [c.body, c.local]);
    expect(local).toEqual([
      [PLANET, true],
      [MOON, true],
    ]);
  });

  it("gives no candidate for a body that is not lit", () => {
    const star: ViewBody = aBody({ id: `${FIXTURE_SYSTEM}.0000`, kind: "star", parent: null });
    const scene = { ...SCENE, bodies: [...SCENE.bodies, star] };
    const bodies = viewFieldCandidates(viewAt(5e6, "photorealistic", { scene }), () => null);
    expect(bodies.map((c) => c.body)).toEqual([PLANET, MOON]);
  });
});

describe("the terrain session's fields (R10.T16.a)", () => {
  it("asks for a view's local body with no header held", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    expect(
      h
        .socket()
        .requestsOfKind("surface_field")
        .map(({ body }) => body),
    ).toEqual([{ kind: "surface_field", universe: UNIVERSE, body: MOON, have_revision: null }]);
  });

  it("asks for no body that is no view's local body while no header is held", async () => {
    const h = await harness();
    // Inside the range the moon's relief would give, but no header is held to give it.
    h.session.noteView(PRIMARY, viewAt(2e7));
    h.session.step();
    expect(h.socket().requestsOfKind("surface_field")).toEqual([]);
  });

  it("asks for a view's readout body as for its local body, the camera far beyond its frame (R10.T12)", async () => {
    const h = await harness();
    // Beyond the moon's frame, and beyond its range with no header held to give it.
    h.session.noteView(PRIMARY, { ...viewAt(7e7), readoutBody: MOON });
    h.session.step();
    expect([
      [...h.session.wanted],
      h
        .socket()
        .requestsOfKind("surface_field")
        .map(({ body }) => body.body),
    ]).toEqual([[MOON], [MOON]]);
  });

  it("keeps a held body wanted while a view is within its hand-over range", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteView(PRIMARY, viewAt(2e7));
    h.session.step();
    expect(h.session.survey(MOON)?.status.reason).toBe("in-range");
  });

  it("keeps a held body inside twice its range, and releases it once every view is beyond", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteView(PRIMARY, viewAt(5e7));
    h.session.step();
    const inBand = [...h.session.wanted];
    h.session.noteView(PRIMARY, viewAt(7e7));
    h.session.step();
    expect([inBand, [...h.session.wanted]]).toEqual([[MOON], []]);
  });

  it("drops a released body's field from every worker", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteView(PRIMARY, viewAt(7e7));
    h.session.step();
    expect(h.workers.map((w) => w.fieldMessages.at(-1)?.kind)).toEqual([
      "field-release",
      "field-release",
    ]);
  });

  it("keeps a body for a photorealistic view within its 1 px range", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteView(PRIMARY, viewAt(2e7, "photorealistic"));
    h.session.step();
    expect([...h.session.wanted]).toEqual([MOON]);
  });

  it("releases a body for a wireframe view at the same place, beyond twice its 4 px range", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteView(PRIMARY, viewAt(2e7, "wireframe"));
    h.session.step();
    expect([...h.session.wanted]).toEqual([]);
  });

  it("keeps every field while it holds no view's report, as a loop starting again on it does (S2)", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteView(PRIMARY, viewAt(2e7));
    h.session.step();
    // The loop's cleanup forgets the primary; the new loop steps before its first report.
    h.session.forgetView(PRIMARY);
    h.session.step();
    h.session.noteView(PRIMARY, viewAt(2e7));
    h.session.step();
    expect([
      [...h.session.wanted],
      h.socket().requestsOfKind("surface_field").length,
      h.workers.map((w) => w.fieldMessages.filter((m) => m.kind === "field-release").length),
    ]).toEqual([[MOON], 1, [0, 0]]);
  });

  describe("a body held by one view of two", () => {
    /** The moon held, then judged by a wireframe primary and a photorealistic instrument. */
    async function twoViews(): Promise<Awaited<ReturnType<typeof harness>>> {
      const h = await harness();
      await holdMoon(h);
      h.session.noteView(PRIMARY, viewAt(2e7, "wireframe"));
      h.session.noteView(INSTRUMENT, viewAt(2e7, "photorealistic"));
      h.session.step();
      return h;
    }

    it("is kept while any view holds it", async () => {
      const h = await twoViews();
      expect([...h.session.wanted]).toEqual([MOON]);
    });

    it("is judged without a view the session forgets", async () => {
      const h = await twoViews();
      h.session.forgetView(INSTRUMENT);
      h.session.step();
      expect([...h.session.wanted]).toEqual([]);
    });
  });

  it("asks a body's delta alone at a revision rise on the scene topic", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    expect(
      h
        .socket()
        .requestsOfKind("surface_field")
        .map(({ body }) => body.have_revision),
    ).toEqual([null, 1]);
  });

  it("asks a held field again with its revision when the link is back", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.setLinkUp(false);
    h.session.setLinkUp(true);
    expect(latestFieldAsk(h.socket())).toMatchObject({ body: MOON, have: 1 });
  });

  it("marks its survey stale while the link is down", async () => {
    const h = await harness();
    await holdMoon(h);
    const live = h.session.survey(MOON)?.stale;
    h.session.setLinkUp(false);
    expect([live, h.session.survey(MOON)?.stale]).toEqual([false, true]);
  });

  it("keeps a held survey stale, with its typed failure, once a later ask is refused", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    h.socket().serverRejects(latestFieldAsk(h.socket()).id, {
      code: "unknown_body",
      message: "no such body",
      field: "body",
    });
    await settle();
    const survey = h.session.survey(MOON);
    expect([survey?.stale, survey?.header, survey?.status.failure]).toEqual([
      true,
      moonLikeHeader(MOON),
      { kind: "rejected", code: "unknown_body", message: "no such body", refusal: true },
    ]);
  });

  it("gives a wanted body's survey: its status, coverage and header", async () => {
    const h = await harness();
    await holdMoon(h);
    const survey = h.session.survey(MOON);
    expect([survey?.status.pending, survey?.coverage?.revision, survey?.header]).toEqual([
      false,
      1,
      moonLikeHeader(MOON),
    ]);
  });

  it("gives no survey of a body not wanted", async () => {
    const h = await harness();
    expect(h.session.survey(PLANET)).toBeNull();
  });

  it("asks a held field again with its revision on a resubscription of the scene", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.resync();
    expect(latestFieldAsk(h.socket())).toMatchObject({ body: MOON, have: 1 });
  });
});

describe("the terrain session's detail seeds and class maps (R10.T16.a)", () => {
  it("asks a body's detail seed of its record once a want, at its scene's time", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    h.session.step();
    expect(
      h
        .socket()
        .requestsOfKind("body_detail")
        .map(({ body }) => body),
    ).toEqual([
      { kind: "body_detail", universe: UNIVERSE, body: MOON, time: SCENE.time, detail: "full" },
    ]);
  });

  it("asks the detail seed again when the body is wanted afresh", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    // Out of the moon's frame with no header held: it is wanted no more.
    h.session.noteView(PRIMARY, viewAt(7e7));
    h.session.step();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    expect(h.socket().requestsOfKind("body_detail")).toHaveLength(2);
  });

  it("asks the class map with the detail seed its record answers", async () => {
    const h = await harness();
    await holdMoon(h);
    h.socket().serverAnswers("body_detail", () => detailWithSeed(SEED));
    await settle();
    const asked = h.workers.flatMap((w) => w.pendingClassMaps.map((q) => [q.body, q.detailSeed]));
    expect([asked, h.session.detailSeed(MOON)]).toEqual([[[MOON, SEED]], SEED]);
  });

  it("asks no class map of a body whose hooks are not modelled", async () => {
    const h = await harness();
    await holdMoon(h);
    h.socket().serverAnswers("body_detail", () => detailWithSeed(null));
    await settle();
    answerBlock(workerAt(h, 0), 0, blockChange(5, [0, 2]));
    await settle();
    expect(h.workers.flatMap((w) => w.pendingClassMaps)).toEqual([]);
  });

  it("takes a kept scene's body's own detail seed, asking no record", async () => {
    const h = await harness();
    const [planet, moon] = SCENE.bodies;
    if (planet === undefined || moon === undefined) {
      throw new Error("the scene holds the planet and its moon");
    }
    const scene: ViewScene = { ...SCENE, bodies: [planet, { ...moon, detailSeed: SEED }] };
    h.session.noteView(PRIMARY, viewAt(5e6, "photorealistic", { scene }));
    h.session.step();
    expect([h.socket().requestsOfKind("body_detail"), h.session.detailSeed(MOON)]).toEqual([
      [],
      SEED,
    ]);
  });

  it("asks a detail seed the link cut off again once it returns", async () => {
    const h = await harness();
    h.session.setLinkUp(false);
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    const whileDown = h.socket().requestsOfKind("body_detail").length;
    h.session.setLinkUp(true);
    expect([whileDown, h.socket().requestsOfKind("body_detail").length]).toEqual([0, 1]);
  });

  it("gives the views the class map it holds", async () => {
    const h = await harness();
    await holdMoon(h);
    h.socket().serverAnswers("body_detail", () => detailWithSeed(SEED));
    await settle();
    const query = h.workers.flatMap((w) => w.pendingClassMaps.map((q) => ({ w, q })))[0];
    query?.w.reply({ kind: "class-map", id: query.q.id, body: MOON, map: oneTexelMap() });
    await settle();
    const elsewhere = lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon");
    expect(h.session.classMaps.classMapOf(MOON, elsewhere)?.kind).toBe("class-map");
  });

  it("gives the views the relief of a body whose header and survey it holds", async () => {
    const h = await harness();
    await holdMoon(h);
    expect([h.session.reliefs.reliefM(MOON), h.session.reliefs.reliefM(PLANET)]).toEqual([
      18_000,
      null,
    ]);
  });
});

describe("the terrain session's coverage masks (R10.T16.a)", () => {
  it("puts a block's surveyed cells in the body's mask before the store's answer", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    sendBlocks(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [10, 12]));
    const mask = maskWritten(h, MOON);
    expect([
      h.session.survey(MOON)?.coverage ?? null,
      h.session.coverageMask(MOON) === null,
      [9, 10, 11, 12].map((cell) => mask?.[maskTexelOf(5, cell)]),
    ]).toEqual([null, false, [0, 255, 255, 0]]);
  });

  it("adds the store's answered cells to the cells the blocks surveyed", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    const { id } = latestFieldAsk(h.socket());
    const bulk = sendBlocks(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [10, 12]));
    h.socket().serverResponds(
      id,
      readyField(moonLikeHeader(MOON), 1, fieldCover([0, 2, 30]), bulk),
    );
    await settle();
    const mask = maskWritten(h, MOON);
    expect([0, 1, 2, 10, 11].map((cell) => mask?.[maskTexelOf(5, cell)])).toEqual([
      255, 255, 0, 255, 255,
    ]);
  });

  it("forgets a mask's cells once the workers drop the body's field", async () => {
    const h = await harness();
    await holdMoon(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [0, 2]));
    const held = h.session.coverageMask(MOON) === null;
    h.session.noteView(PRIMARY, viewAt(7e7));
    h.session.step();
    expect([held, h.session.coverageMask(MOON), h.tally.liveBytes("coarse-field-gpu")]).toEqual([
      false,
      null,
      0,
    ]);
  });

  it("makes the masks again with the same cells after a device loss", async () => {
    const h = await harness();
    await holdMoon(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [10, 12]));
    const before = maskWritten(h, MOON);
    const written = h.engine.textureWritten.length;
    h.engine.restore();
    expect([h.engine.textureWritten.length - written, maskWritten(h, MOON)]).toEqual([1, before]);
  });

  it("asks for no field and posts no block again after a device loss", async () => {
    const h = await harness();
    await holdMoon(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [10, 12]));
    const asked = h.socket().requestsOfKind("surface_field").length;
    const posted = h.workers.map((w) => w.fieldMessages.length);
    h.engine.restore();
    h.session.step();
    expect([
      h.socket().requestsOfKind("surface_field").length,
      h.workers.map((w) => w.fieldMessages.length),
    ]).toEqual([asked, posted]);
  });
});

describe("the terrain session's pool, memory and teardown (R10.T16.a)", () => {
  it("runs one pool of Design note 11's count, whatever the views and bodies", async () => {
    vi.spyOn(globalThis.navigator, "hardwareConcurrency", "get").mockReturnValue(12);
    const h = await harness({ workers: "default" });
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.noteView(INSTRUMENT, viewAt(8e6, "wireframe", { centreM: FIXTURE_PLANET_CENTRE_M }));
    h.session.step();
    // Each view's local body, in the order the scene lists them.
    expect([h.workers.length, h.session.workerCount, [...h.session.wanted]]).toEqual([
      3,
      3,
      [PLANET, MOON],
    ]);
  });

  it("reports the workers' copies, the pool's kept blocks and its GPU bytes by category", async () => {
    const h = await harness();
    await holdMoon(h);
    for (const worker of h.workers) {
      answerBlock(worker, 0, blockChange(5, [0, 2]), 50);
    }
    expect(h.session.memory()).toEqual({
      workerFieldBytes: 100,
      keptFieldBytes: 50,
      // One byte a level-5 cell a face (Design note 12).
      gpuBytes: new Map([["coarse-field-gpu", 6 * 4 ** 5]]),
    });
  });

  it("leaves no worker, request, timer, listener or GPU handle once disposed", async () => {
    vi.useFakeTimers();
    const h = await harness();
    const timers = vi.getTimerCount();
    await holdMoon(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [0, 2]));
    h.socket().serverAnswers("body_detail", () => detailWithSeed(SEED));
    await settle();
    const query = h.workers.flatMap((w) => w.pendingClassMaps.map((q) => ({ w, q })))[0];
    query?.w.reply({ kind: "class-map", id: query.q.id, body: MOON, map: oneTexelMap() });
    // The planet, the instrument's local body: its field refused busy (a retry timer) and its
    // detail seed asked (a request out).
    h.session.noteView(INSTRUMENT, viewAt(8e6, "wireframe", { centreM: FIXTURE_PLANET_CENTRE_M }));
    h.session.step();
    h.socket().serverRejects(latestFieldAsk(h.socket()).id, {
      code: "queue_full",
      message: "busy",
      field: null,
    });
    await settle();
    const before = [vi.getTimerCount() - timers, h.tally.liveBytes("coarse-field-gpu") > 0];
    const detailIds = new Set(
      h
        .socket()
        .requestsOfKind("body_detail")
        .map(({ id }) => id),
    );
    h.session.dispose();
    const textures = h.engine.counts.textures;
    h.engine.restore();
    expect([
      before,
      vi.getTimerCount() - timers,
      h.workers.map((w) => [w.terminated, w.listening.message, w.listening.error]),
      h
        .socket()
        .cancelledIds()
        .filter((id) => detailIds.has(id)).length,
      h.tally.liveBytes("coarse-field-gpu"),
      h.engine.counts.textures - textures,
    ]).toEqual([
      [1, true],
      0,
      [
        [true, 0, 0],
        [true, 0, 0],
      ],
      1,
      0,
      0,
    ]);
  });

  describe("disposed with a body's class map and mask made", () => {
    /** The moon held with its class map answered, then the session disposed. */
    async function disposedHolding(): Promise<{
      readonly h: Awaited<ReturnType<typeof harness>>;
      readonly destroyed: ReadonlyArray<string>;
    }> {
      const h = await harness();
      await holdMoon(h);
      answerBlock(workerAt(h, 0), 0, blockChange(5, [0, 2]));
      h.socket().serverAnswers("body_detail", () => detailWithSeed(SEED));
      await settle();
      const query = h.workers.flatMap((w) => w.pendingClassMaps.map((q) => ({ w, q })))[0];
      query?.w.reply({ kind: "class-map", id: query.q.id, body: MOON, map: oneTexelMap() });
      await settle();
      const from = h.allocations.length;
      h.session.dispose();
      const destroyed = h.allocations
        .slice(from)
        .flatMap((event) => (event.kind === "destroyed" ? [event.name] : []));
      return { h, destroyed };
    }

    it("releases the class maps before the masks", async () => {
      const { destroyed } = await disposedHolding();
      expect(destroyed).toEqual([`class map ${MOON}`, `coverage mask ${MOON}`]);
    });

    it("releases the fields before the workers stop", async () => {
      const { h } = await disposedHolding();
      expect(h.workers.map((w) => [w.fieldMessages.at(-1)?.kind, w.terminated])).toEqual([
        ["field-release", true],
        ["field-release", true],
      ]);
    });
  });

  it("does nothing once disposed", async () => {
    const h = await harness();
    h.session.dispose();
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    h.session.setLinkUp(true);
    h.session.resync();
    expect(h.socket().requestsOfKind("surface_field")).toEqual([]);
  });
});

/** A line of sight down onto the moon from 2,000 km, body-fixed. */
const SIGHT = {
  originM: vec3(0, 2e6, 0),
  direction: vec3(0, -1, 0),
  maxM: 1e7,
} as const;
const OPTIONS = { slopeBaselineM: 2, padWidthM: 1 } as const;
/** The instant the view lights the moon at: a year and a day after the epoch. */
const LIT: UniverseTime = { seconds: 31_644_000, nanos: 250_000_000 };

/** The record of a moon of the fixture's planet: the Earth's, of kind moon about the planet. */
function moonDetail(): ResponseFor<"body_detail"> {
  const detail = detailWithSeed(SEED);
  return {
    ...detail,
    record: {
      ...detail.record,
      kind: { type: "moon", origin: "regular" },
      parent: { type: "body", id: PLANET },
    },
  };
}

/** The planet's record: the Earth's, its orbit's eccentricity and phase set apart from the moon's. */
function planetDetail(): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  const { orbit } = detail.record;
  if (orbit.state !== "ok") {
    throw new Error("the fixture's Earth has an orbit");
  }
  return {
    ...detail,
    record: {
      ...detail.record,
      orbit: {
        state: "ok",
        value: {
          ...orbit.value,
          orbit: { ...orbit.value.orbit, eccentricity: 0.3, mean_anomaly_at_epoch_rad: 4 },
        },
      },
    },
  };
}

/** The record's own orbit as the client propagates it. */
function orbitOf(detail: ResponseFor<"body_detail">): KeplerOrbit {
  const { orbit } = detail.record;
  if (orbit.state !== "ok") {
    throw new Error("the record has an orbit");
  }
  return toKeplerOrbit(orbit.value.orbit);
}

/** The moon held, its record answered by `detail`. */
async function holdWith(h: Harness, detail: ResponseFor<"body_detail">): Promise<void> {
  await holdMoon(h);
  h.socket().serverAnswers("body_detail", () => detail);
  await settle();
}

/** The ground-readout queries the workers were posted, each with its worker, in posting order. */
function readoutsAsked(
  h: Harness,
): { readonly w: ScriptedHeightWorker; readonly q: GroundReadoutRequest }[] {
  return h.workers
    .flatMap((w) => w.pendingGroundReadouts.map((q) => ({ w, q })))
    .toSorted((a, b) => a.q.id - b.q.id);
}

describe("the terrain session's ground readouts (R10.T12)", () => {
  it("asks one worker for the readout with the body's seed and the season of its own orbit", async () => {
    const h = await harness();
    const detail = detailWithSeed(SEED);
    await holdWith(h, detail);
    const asked = h.session.groundReadout(PRIMARY, MOON, SIGHT, LIT, OPTIONS);
    const posted = readoutsAsked(h);
    expect(posted.map(({ q }) => q)).toEqual([
      {
        kind: "ground-readout",
        id: posted[0]?.q.id,
        body: MOON,
        detailSeed: SEED,
        originM: SIGHT.originM,
        direction: SIGHT.direction,
        maxM: SIGHT.maxM,
        slopeBaselineM: 2,
        padWidthM: 1,
        seasonMeanAnomalyRad: meanAnomalyAt(orbitOf(detail), LIT),
      },
    ]);
    const readout = { kind: "no-ground" } as const;
    const first = posted[0];
    first?.w.reply({ kind: "ground-readout", id: first.q.id, body: MOON, readout });
    await expect(asked).resolves.toEqual(readout);
  });

  it("takes a moon's season from its planet's orbit, asking the planet's record once", async () => {
    const h = await harness();
    await holdMoon(h);
    h.socket().serverAnswers("body_detail", () => moonDetail());
    await settle();
    const planetAsks = h
      .socket()
      .requestsOfKind("body_detail")
      .map(({ body }) => body)
      .filter((body) => body.body === PLANET);
    expect(planetAsks).toEqual([
      { kind: "body_detail", universe: UNIVERSE, body: PLANET, time: SCENE.time, detail: "full" },
    ]);
    // Until the planet's record answers, no season is known.
    const before = h.session.groundReadout(PRIMARY, MOON, SIGHT, LIT, OPTIONS);
    h.socket().serverAnswers("body_detail", () => planetDetail());
    await settle();
    const after = h.session.groundReadout(PRIMARY, MOON, SIGHT, LIT, OPTIONS);
    const posted = readoutsAsked(h);
    expect(posted.map(({ q }) => q.seasonMeanAnomalyRad)).toEqual([
      undefined,
      meanAnomalyAt(orbitOf(planetDetail()), LIT),
    ]);
    expect(meanAnomalyAt(orbitOf(planetDetail()), LIT)).not.toBe(
      meanAnomalyAt(orbitOf(moonDetail()), LIT),
    );
    const readout = { kind: "no-ground" } as const;
    for (const { w, q } of posted) {
      w.reply({ kind: "ground-readout", id: q.id, body: MOON, readout });
    }
    await expect(Promise.all([before, after])).resolves.toEqual([readout, readout]);
  });

  it("gives no season to a kept scene's body, whose record it does not ask", async () => {
    const h = await harness();
    const [planet, moon] = SCENE.bodies;
    if (planet === undefined || moon === undefined) {
      throw new Error("the scene holds the planet and its moon");
    }
    const scene: ViewScene = { ...SCENE, bodies: [planet, { ...moon, detailSeed: SEED }] };
    h.session.noteView(PRIMARY, viewAt(5e6, "photorealistic", { scene }));
    h.session.step();
    await answerField(h);
    const asked = h.session.groundReadout(PRIMARY, MOON, SIGHT, LIT, OPTIONS);
    const posted = readoutsAsked(h);
    expect([
      h.socket().requestsOfKind("body_detail"),
      posted.map(({ q }) => [q.detailSeed, q.seasonMeanAnomalyRad]),
    ]).toEqual([[], [[SEED, undefined]]]);
    const readout = { kind: "no-ground" } as const;
    for (const { w, q } of posted) {
      w.reply({ kind: "ground-readout", id: q.id, body: MOON, readout });
    }
    await expect(asked).resolves.toEqual(readout);
  });

  it("rejects a malformed time, rather than throwing, and asks no worker", async () => {
    const h = await harness();
    await holdWith(h, detailWithSeed(SEED));
    const malformed: UniverseTime = { seconds: 1.5, nanos: 0 };
    const asked = h.session.groundReadout(PRIMARY, MOON, SIGHT, malformed, OPTIONS);
    await expect(asked).rejects.toThrow(RangeError);
    expect(readoutsAsked(h)).toEqual([]);
  });

  it("asks a moon's planet's record the link cut off again once it returns", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.setLinkUp(false);
    h.socket().serverAnswers("body_detail", () => moonDetail());
    await settle();
    const asked = (): number =>
      h
        .socket()
        .requestsOfKind("body_detail")
        .filter(({ body }) => body.body === PLANET).length;
    const whileDown = asked();
    h.session.setLinkUp(true);
    expect([whileDown, asked()]).toEqual([0, 1]);
  });

  it("answers null once disposed, for a view it holds no report of, a body not wanted, before the seed and before the field", async () => {
    const h = await harness();
    const ask = (view = PRIMARY, body = MOON): Promise<unknown> =>
      h.session.groundReadout(view, body, SIGHT, LIT, OPTIONS);
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    // Wanted, its seed asked and its field not yet answered.
    const beforeSeed = ask();
    h.socket().serverAnswers("body_detail", () => detailWithSeed(SEED));
    await settle();
    const beforeField = ask();
    await answerField(h);
    const notWanted = ask(PRIMARY, PLANET);
    const unknownView = ask(INSTRUMENT);
    h.session.dispose();
    const disposed = ask();
    await expect(
      Promise.all([beforeSeed, beforeField, notWanted, unknownView, disposed]),
    ).resolves.toEqual([null, null, null, null, null]);
    expect(readoutsAsked(h)).toEqual([]);
  });

  it("cancels a moon's planet's record still asked when disposed", async () => {
    const h = await harness();
    await holdMoon(h);
    h.socket().serverAnswers("body_detail", () => moonDetail());
    await settle();
    const planetAsk = h
      .socket()
      .requestsOfKind("body_detail")
      .find(({ body }) => body.body === PLANET);
    h.session.dispose();
    expect(h.socket().cancelledIds()).toContain(planetAsk?.id);
  });
});

/** Fails every height worker of the pool's two places, and each replacement, until both are lost. */
function loseEveryWorker(h: Harness): void {
  for (let round = 0; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
    for (const worker of h.workers.slice(-2)) {
      worker.fail();
    }
  }
}

describe("the terrain session's fault for the terrain readout (R10.T12; decision-r10-t12-leans §5)", () => {
  it("raises none while its workers live and no computation has kept failing", async () => {
    const h = await harness();
    await holdMoon(h);
    expect(h.session.terrainFault).toBeNull();
  });

  it("gives the computation's fault once a guarded computation has kept failing", async () => {
    const h = await harness();
    await holdMoon(h);
    h.session.failComputation();
    expect(h.session.terrainFault).toBe(TERRAIN_COMPUTATION_FAILED);
  });

  it("gives TERRAIN NOT AVAILABLE once every height worker is lost", async () => {
    const h = await harness();
    await holdMoon(h);
    loseEveryWorker(h);
    expect(h.session.terrainFault).toBe(TERRAIN_NOT_AVAILABLE);
  });

  it("shows the fault on a view whose report names a readout body, though it hands nothing over", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, { ...viewAt(5e6), readoutBody: MOON });
    h.session.step();
    await answerField(h);
    h.session.failComputation();
    expect(new Map(h.session.terrainFaults)).toEqual(
      new Map([[PRIMARY, TERRAIN_COMPUTATION_FAILED]]),
    );
  });

  it("shows the fault on a view whose readout body's survey is still pending (decision-r10-t12-commit3 §6)", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, { ...viewAt(5e6), readoutBody: MOON });
    h.session.step();
    h.session.failComputation();
    expect({
      pending: h.session.survey(MOON)?.status.pending,
      views: new Map(h.session.terrainFaults),
    }).toEqual({ pending: true, views: new Map([[PRIMARY, TERRAIN_COMPUTATION_FAILED]]) });
  });

  it("shows no fault on a view whose readout body's terrain is not modelled (decision-r10-t12-commit3 §6)", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, { ...viewAt(5e6), readoutBody: MOON });
    h.session.step();
    h.socket().serverResponds(latestFieldAsk(h.socket()).id, {
      kind: "surface_field",
      state: "not_modelled",
      section: "surface",
    });
    await settle();
    h.session.failComputation();
    expect({
      notModelled: h.session.survey(MOON)?.status.notModelled,
      fault: h.session.terrainFault,
      views: [...h.session.terrainFaults.keys()],
    }).toEqual({ notModelled: "surface", fault: TERRAIN_COMPUTATION_FAILED, views: [] });
  });

  it("shows no fault on a view that names no readout body and hands nothing over", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, { ...viewAt(5e6), readoutBody: null });
    h.session.step();
    await answerField(h);
    h.session.failComputation();
    expect({
      fault: h.session.terrainFault,
      views: [...h.session.terrainFaults.keys()],
    }).toEqual({ fault: TERRAIN_COMPUTATION_FAILED, views: [] });
  });
});
