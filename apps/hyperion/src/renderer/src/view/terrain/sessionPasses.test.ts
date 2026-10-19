/**
 * The terrain session's passes (plan R10, R10.T16.b): one a handed-over body, shared by the views
 * that hand it over, made at its first hand-over and disposed at its field's release; the mask held
 * behind the rebakes a block names; the fault; the device loss; and the releases, over R03's
 * `FakeWebSocket`, scripted height workers and R01's counting engine.
 */

import type { BodyIdHex, DetailSeedHex, UniverseIdHex } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
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
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  SESSION_MOON,
  SESSION_MOON_RADIUS_M,
  SESSION_SCENE,
  viewAt,
} from "../../test/terrainSessionFixtures";
import { aViewCraft, NO_TURN } from "../../test/viewFixtures";
import { lawFor } from "../appearance/phase";
import { viewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import type { MemoryCategory } from "../engine/memory";
import type { ViewBody, ViewScene } from "../scene/model";
import { TERRAIN_NOT_AVAILABLE } from "./annunciation";
import { fieldCellIndex } from "./coverage";
import { allocationTally } from "./gpu/allocationTally";
import { datumGroundUnder } from "./grounded";
import { childKeys, type PatchKey, patchKeyString } from "./patchKey";
import { useTerrainFaults } from "../../displays/view/useTerrainSession";
import { type TerrainViewReport, sceneContacts, TerrainSession } from "./session";
import type { TerrainView, TerrainViewFrameInput } from "./terrainPass";
import { PALETTE_NONE, type TerrainLayers } from "./workers/messages";
import { MAX_CONSECUTIVE_WORKER_FAILURES } from "./workers/pool";

const UNIVERSE: UniverseIdHex = "000000000000002a";
const MOON: BodyIdHex = SESSION_MOON;
const SEED: DetailSeedHex = "0123456789abcdef";
const PRIMARY = viewId("primary");
const INSTRUMENT = viewId("instrument 1");
const THIRD = viewId("instrument 2");

/** The field's level, `moonLikeHeader`'s. */
const FIELD_LEVEL = 5;

/** The scene's moon, which every test here needs. */
function sceneMoon(scene: ViewScene): ViewBody {
  const moon = scene.bodies.find((body) => body.id === MOON);
  if (moon === undefined) {
    throw new Error("the scene holds its moon");
  }
  return moon;
}

/** `scene` with its moon `moon`. */
function withMoon(scene: ViewScene, moon: ViewBody): ViewScene {
  return { ...scene, bodies: scene.bodies.map((body) => (body.id === MOON ? moon : body)) };
}

/** The scene with the moon turning (its rotation the identity) and carrying its own detail seed. */
const SCENE: ViewScene = withMoon(SESSION_SCENE, {
  ...sceneMoon(SESSION_SCENE),
  rotation: IDENTITY_ROTATION,
  detailSeed: SEED,
});

/** A photorealistic view `distanceM` above the moon's north pole that hands the moon over. */
function handing(
  distanceM: number,
  overrides: Partial<TerrainViewReport> = {},
  scene: ViewScene = SCENE,
): TerrainViewReport {
  return {
    ...viewAt(distanceM, "photorealistic", { scene }),
    regimes: new Map([[MOON, "terrain"]]),
    ...overrides,
  };
}

/** The same view handing nothing over. */
function notHanding(distanceM: number): TerrainViewReport {
  return { ...viewAt(distanceM, "photorealistic", { scene: SCENE }), regimes: new Map() };
}

/** Resolves after `hops` turns of the microtask queue. */
function microtasks(hops: number): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(() => microtasks(hops - 1));
}

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
  readonly tally: ReturnType<typeof allocationTally>;
}

async function harness(): Promise<Harness> {
  const hook = renderHook(() => useServerLink(), { wrapper: ServerLinkHarness });
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  const engine = await countingRenderEngine();
  const tally = allocationTally(engine);
  const workers: ScriptedHeightWorker[] = [];
  const session = new TerrainSession({
    engine,
    universe: UNIVERSE,
    requests: hook.result.current.requests,
    linkUp: true,
    createWorker: () => {
      const worker = new ScriptedHeightWorker();
      workers.push(worker);
      return worker;
    },
    workers: 2,
  });
  return {
    socket: () => FakeWebSocket.latest(),
    link: () => hook.result.current,
    engine,
    workers,
    session,
    tally,
  };
}

/** Answers the latest field request with one block, at `revision`, over `cells` surveyed. */
async function answerField(
  h: Harness,
  revision = 1,
  cells: ReadonlyArray<number> = [0, 1],
): Promise<void> {
  const { id } = latestFieldAsk(h.socket());
  const bulk = sendFieldFrames(h.socket(), id, fieldPayload([fieldBlock(50, 1)]));
  const ranges = cells.map((cell) => [cell, cell + 1, 30] as const);
  h.socket().serverResponds(
    id,
    readyField(moonLikeHeader(MOON), revision, fieldCover(...ranges), bulk),
  );
  await settle();
}

/** The moon handed over by the primary from 5,000 km, its field answered, its pass made. */
async function handedOver(h: Harness): Promise<void> {
  h.session.noteView(PRIMARY, handing(5e6));
  h.session.step();
  await answerField(h);
  h.session.step();
  await settle();
}

/** The field-level cells under `key`, by `fieldCellIndex`. */
function cellsUnder(key: PatchKey): number[] {
  if (key.level >= FIELD_LEVEL) {
    const shift = key.level - FIELD_LEVEL;
    return [
      fieldCellIndex({ face: key.face, level: FIELD_LEVEL, i: key.i >> shift, j: key.j >> shift }),
    ];
  }
  return childKeys(key).flatMap(cellsUnder);
}

/** What each bake held of the survey, by key: the cells under it surveyed when it was baked. */
type Held = Map<string, ReadonlySet<number>>;

/** A body's layers over a survey of `surveyed`: complete, its vertices outside it named. */
function layersOver(key: PatchKey, surveyed: ReadonlySet<number>): TerrainLayers {
  const unsurveyed = cellsUnder(key).filter((cell) => !surveyed.has(cell));
  const palette = new Uint8Array(16).fill(PALETTE_NONE);
  return {
    palette,
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(65 * 65),
    fieldLevel: FIELD_LEVEL,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: Uint32Array.from(unsurveyed.toSorted((a, b) => a - b)),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/**
 * Answers every bake the workers hold over the survey `surveyed`, a bake sized as its settings
 * ask, and notes what each held; returns how many it answered.
 */
function answerBakes(h: Harness, surveyed: ReadonlySet<number>, held: Held): number {
  let answered = 0;
  for (const worker of h.workers) {
    for (const request of worker.pendingBakes) {
      const { key, settings } = request;
      const n = settings.normals === "double" ? 129 : 65;
      worker.reply({
        kind: "baked",
        id: request.id,
        bake: {
          key,
          generation: request.generation,
          originM: vec3(0, 0, SESSION_MOON_RADIUS_M),
          heights: new Float32Array(65 * 65 * 2),
          offsets: settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
          normals: new Float16Array(n * n * 2),
          heightRangeM: [0, 0],
          boundingRadiusM: 1,
          originHeightM: 0,
          skirtDepthM: 1,
          layers: layersOver(key, surveyed),
        },
      });
      held.set(patchKeyString(key), new Set(cellsUnder(key).filter((cell) => surveyed.has(cell))));
      answered += 1;
    }
  }
  return answered;
}

/** The primary's camera as its pass draws it: 5,000 km above the moon's north pole. */
const PRIMARY_VIEW: TerrainView = {
  rotation: IDENTITY_ROTATION,
  cameraM: vec3(0, 0, 5e6),
  orientation: NO_TURN,
  fovXRad: Math.PI / 3,
  viewport: { widthPx: 1920, heightPx: 1080 },
};

function frameAt(nowMs: number): TerrainViewFrameInput {
  return {
    view: PRIMARY_VIEW,
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    lighting: {
      lights: [],
      occluders: [],
      secondaries: [],
      elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      exposureScale: 1e-5,
    },
    nowMs,
  };
}

/** The cells the latest write of the moon's coverage mask holds surveyed. */
function maskCells(h: Harness): Set<number> {
  const data = h.engine.textureWritten.findLast((w) => w.texture === `coverage mask ${MOON}`)?.data;
  const cells = new Set<number>();
  const n = 2 ** FIELD_LEVEL;
  data?.forEach((byte, at) => {
    if (byte !== 0) {
      const i = at % n;
      const j = Math.floor(at / n) % n;
      const face = Math.floor(at / (n * n));
      if (isFace(face)) {
        cells.add(fieldCellIndex({ face, level: FIELD_LEVEL, i, j }));
      }
    }
  });
  return cells;
}

/**
 * The mask's invariant (decision-r10-t16-contracts §2) in the primary, where the mask is given:
 * each cell it holds surveyed lies under a drawn patch whose bake holds it; the cells that break it.
 */
function brokenCells(h: Harness, held: Held, nowMs: number): number[] {
  const frame = h.session.drawTerrain(PRIMARY, MOON, frameAt(nowMs));
  if (frame === null || !frame.maskReady) {
    return [];
  }
  const drawn = h.session.pass(MOON)?.lastStep?.drawSet;
  const covered = new Set<number>();
  for (const { keyString } of drawn?.patches ?? []) {
    for (const cell of held.get(keyString) ?? []) {
      covered.add(cell);
    }
  }
  // Only the cells under the primary's view: the drawn patches cover what it sees.
  const seen = new Set((drawn?.patches ?? []).flatMap(({ patch }) => cellsUnder(patch.key)));
  return [...maskCells(h)].filter((cell) => seen.has(cell) && !covered.has(cell));
}

/** Whether `n` is a face's index. */
function isFace(n: number): n is PatchKey["face"] {
  return n === 0 || n === 1 || n === 2 || n === 3 || n === 4 || n === 5;
}

/** Fails every worker the pool runs until each place is given up. */
function loseEveryWorker(h: Harness): void {
  for (let round = 0; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
    for (const worker of h.workers.slice(-2)) {
      worker.fail();
    }
  }
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the terrain session's passes (R10.T16.b; decision-r10-live-path, D3 and D4)", () => {
  it("makes a body's pass at its first hand-over, one for every view that hands it over", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, notHanding(5e6));
    h.session.step();
    await answerField(h);
    h.session.step();
    expect(h.session.passBodies.size).toBe(0);
    h.session.noteView(PRIMARY, handing(5e6));
    h.session.noteView(INSTRUMENT, handing(6e6, { streamPriority: "secondary" }));
    h.session.step();
    const pass = h.session.pass(MOON);
    h.session.step();
    expect([...h.session.passBodies]).toEqual([MOON]);
    expect(h.session.pass(MOON)).toBe(pass);
    expect([...(pass?.viewIds ?? [])]).toEqual([PRIMARY, INSTRUMENT]);
  });

  it("hands nothing over for a wireframe view, or a body whose rotation is not known", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, handing(5e6, { style: "wireframe" }));
    h.session.step();
    await answerField(h);
    h.session.step();
    const unturned = withMoon(SCENE, { ...sceneMoon(SCENE), rotation: null });
    h.session.noteView(INSTRUMENT, handing(5e6, {}, unturned));
    h.session.step();
    expect(h.session.passBodies.size).toBe(0);
  });

  it("bakes through a lane of its one pool, of the body's lit settings", async () => {
    const h = await harness();
    await handedOver(h);
    const bakes = h.workers.flatMap((w) => w.pendingBakes);
    expect(bakes.length).toBeGreaterThan(0);
    expect(bakes.every((b) => b.settings.source.kind === "body")).toBe(true);
    expect(
      bakes.map((b) =>
        b.settings.source.kind === "body"
          ? [b.settings.source.body, b.settings.source.weights]
          : null,
      )[0],
    ).toEqual([MOON, "baked"]);
    expect(h.workers.length).toBe(2);
  });

  it("keeps a pass handed back, and disposes of it with its handles at its field's release", async () => {
    const h = await harness();
    const before = h.engine.live;
    const bytes = (["height-cache", "other"] as const).map((c) => h.tally.liveBytes(c));
    await handedOver(h);
    answerBakes(h, new Set([0, 1]), new Map());
    h.session.step();
    h.session.drawTerrain(PRIMARY, MOON, frameAt(0));
    // Handed back: the pass stays, its view's own draw released.
    h.session.noteView(PRIMARY, notHanding(5e6));
    h.session.step();
    expect(h.session.passBodies.size).toBe(1);
    // Beyond twice the range: the field and the pass go.
    h.session.noteView(PRIMARY, notHanding(7e7));
    h.session.step();
    expect(h.session.passBodies.size).toBe(0);
    expect(h.session.drawTerrain(PRIMARY, MOON, frameAt(16))).toBeNull();
    expect(h.engine.live).toEqual(before);
    expect((["height-cache", "other"] as const).map((c) => h.tally.liveBytes(c))).toEqual(bytes);
  });

  it("leaves the tallies where they started after 50 hand-overs and releases", async () => {
    const h = await harness();
    const categories: ReadonlyArray<MemoryCategory> = ["height-cache", "other", "coarse-field-gpu"];
    const before = h.engine.live;
    const bytes = categories.map((c) => h.tally.liveBytes(c));
    for (let n = 0; n < 50; n += 1) {
      h.session.noteView(PRIMARY, handing(5e6));
      h.session.step();
      // The field is asked afresh each time: answered, the body handed over.
      // oxlint-disable-next-line no-await-in-loop
      await answerField(h, 1);
      h.session.step();
      // oxlint-disable-next-line no-await-in-loop
      await settle();
      expect(h.session.passBodies.size).toBe(1);
      h.session.noteView(PRIMARY, notHanding(7e7));
      h.session.step();
      expect(h.session.passBodies.size).toBe(0);
    }
    expect(h.engine.live).toEqual(before);
    expect(categories.map((c) => h.tally.liveBytes(c))).toEqual(bytes);
  });

  it("disposes of its passes first when disposed, leaving no handle", async () => {
    const h = await harness();
    const before = h.engine.live;
    await handedOver(h);
    h.session.dispose();
    expect(h.engine.live).toEqual(before);
    expect(h.session.passBodies.size).toBe(0);
  });
});

describe("the terrain session's mask behind the rebakes (R10.T16.b; decision D7)", () => {
  it("gives the mask a block's cells only once the rebakes it named have landed, the invariant holding throughout", async () => {
    const h = await harness();
    // The first survey: four cells about the north pole of face +z.
    const first = [
      { i: 15, j: 15 },
      { i: 16, j: 15 },
      { i: 15, j: 16 },
      { i: 16, j: 16 },
    ].map(({ i, j }) => fieldCellIndex({ face: 2, level: FIELD_LEVEL, i, j }));
    const second = [
      { i: 17, j: 15 },
      { i: 17, j: 16 },
    ].map(({ i, j }) => fieldCellIndex({ face: 2, level: FIELD_LEVEL, i, j }));
    h.session.noteView(PRIMARY, handing(5e6));
    h.session.step();
    await answerField(h, 1, first);
    h.session.step();
    await settle();
    const surveyed = new Set(first);
    const held: Held = new Map();
    for (const worker of h.workers) {
      answerBlock(worker, 0, blockChange(FIELD_LEVEL, runsOf(first)));
    }
    const broken: number[] = [];
    let now = 0;
    // Streamed over the first survey: resident patches over it, and stand-ins over their siblings.
    for (let round = 0; round < 6; round += 1) {
      h.session.step();
      answerBakes(h, surveyed, held);
      now += 16;
      broken.push(...brokenCells(h, held, now));
    }
    const pass = h.session.pass(MOON);
    expect(pass?.lastStep?.drawSet.count).toBeGreaterThan(0);
    expect(first.every((cell) => maskCells(h).has(cell))).toBe(true);
    // The delta, at a revision rise: the store's coverage holds the two cells more at once, and is
    // held back from the mask until the workers take its block.
    h.session.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    await answerField(h, 2, second);
    expect(h.session.survey(MOON)?.coverage?.surveyedFraction()).toBeGreaterThan(0);
    expect(second.some((cell) => maskCells(h).has(cell))).toBe(false);
    // Its block surveys them under patches resident and drawn, stand-ins among them: the mask waits
    // for their rebakes.
    for (const worker of h.workers) {
      answerBlock(worker, 1, blockChange(FIELD_LEVEL, runsOf(second)));
    }
    expect(pass?.settled).toBe(false);
    expect(second.some((cell) => maskCells(h).has(cell))).toBe(false);
    now += 16;
    broken.push(...brokenCells(h, held, now));
    for (const cell of second) {
      surveyed.add(cell);
    }
    for (let round = 0; round < 6; round += 1) {
      h.session.step();
      answerBakes(h, surveyed, held);
      now += 16;
      broken.push(...brokenCells(h, held, now));
    }
    expect(pass?.settled).toBe(true);
    expect(second.every((cell) => maskCells(h).has(cell))).toBe(true);
    expect(broken).toEqual([]);
  });

  it("gives the mask a block's cells at once where no pass draws the body", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, notHanding(5e6));
    h.session.step();
    await answerField(h);
    const cell = fieldCellIndex({ face: 2, level: FIELD_LEVEL, i: 3, j: 3 });
    answerBlock(h.workers[0] ?? fail(), 0, blockChange(FIELD_LEVEL, [cell, cell + 1]));
    expect(maskCells(h).has(cell)).toBe(true);
  });
});

/** Fails the test where a value it needs is missing. */
function fail(): never {
  throw new Error("a value the test needs is missing");
}

/** A sorted list of cells as runs of one cell each. */
function runsOf(cells: ReadonlyArray<number>): number[] {
  return cells.toSorted((a, b) => a - b).flatMap((cell) => [cell, cell + 1]);
}

describe("the terrain session's fault (R10.T16.b; decision-r10-t16-contracts §7)", () => {
  it("shows the fault on every view that hands a body over once every worker is lost, and draws nothing", async () => {
    const h = await harness();
    await handedOver(h);
    h.session.noteView(INSTRUMENT, handing(6e6, { streamPriority: "secondary" }));
    h.session.noteView(THIRD, notHanding(6e6));
    h.session.step();
    let told = 0;
    h.session.subscribeFaults(() => {
      told += 1;
    });
    expect(h.session.terrainFaults.size).toBe(0);
    loseEveryWorker(h);
    h.session.step();
    const frame = h.session.drawTerrain(PRIMARY, MOON, frameAt(5_000));
    expect([...h.session.terrainFaults].toSorted()).toEqual([INSTRUMENT, PRIMARY].toSorted());
    expect(told).toBe(1);
    expect([frame?.draw, frame?.fault, frame?.annunciation]).toEqual([
      null,
      TERRAIN_NOT_AVAILABLE,
      null,
    ]);
    // The same set while nothing changes, for `useSyncExternalStore`.
    const faults = h.session.terrainFaults;
    h.session.step();
    expect([h.session.terrainFaults, told]).toEqual([faults, 1]);
  });

  it("reaches the label blocks through useTerrainFaults, and none without a session", async () => {
    const h = await harness();
    await handedOver(h);
    const hook = renderHook(() => useTerrainFaults(h.session));
    const none = renderHook(() => useTerrainFaults(null));
    expect([hook.result.current.size, none.result.current.size]).toEqual([0, 0]);
    act(() => {
      loseEveryWorker(h);
      h.session.step();
    });
    expect([...hook.result.current]).toEqual([PRIMARY]);
  });
});

describe("the terrain session's passes after a device loss (R10.T16.b; decision D8)", () => {
  it("bakes again on a new lane and posts no block or field again", async () => {
    const h = await harness();
    await handedOver(h);
    answerBakes(h, new Set([0, 1]), new Map());
    h.session.step();
    const asked = h.socket().requestsOfKind("surface_field").length;
    const posted = h.workers.map((w) => w.fieldMessages.length);
    const bakes = h.workers.reduce(
      (n, w) => n + w.received.filter((m) => m.kind === "bake").length,
      0,
    );
    h.engine.restore();
    await settle();
    h.session.step();
    expect([
      h.socket().requestsOfKind("surface_field").length,
      h.workers.map((w) => w.fieldMessages.length),
    ]).toEqual([asked, posted]);
    expect(
      h.workers.reduce((n, w) => n + w.received.filter((m) => m.kind === "bake").length, 0),
    ).toBeGreaterThan(bakes);
  });
});

describe("the scene's craft as a pass's contacts (R10.T16.b; R05 Design note 9)", () => {
  const moon = sceneMoon(SCENE);
  const figure = moonLikeHeader(MOON);
  const ground = datumGroundUnder({
    equatorialRadiusM: figure.equatorial_radius_m,
    polarRadiusM: figure.polar_radius_m,
  });
  /** The scene with one craft in the moon's frame `heightM` above its north pole, at `vzMps`. */
  function withCraft(heightM: number, vzMps: number): ViewScene {
    return {
      ...SCENE,
      craft: [
        aViewCraft({
          pose: {
            position: {
              kind: "body",
              body: MOON,
              m: vec3(0, 0, figure.polar_radius_m + heightM),
            },
            attitude: NO_TURN,
          },
          velocityMPerS: vec3(0, 0, vzMps),
        }),
      ],
    };
  }

  it("takes a craft at 900 m and closing as a contact on the ground beneath it, at the hull's radius", () => {
    const contacts = sceneContacts(withCraft(900, -5), moon, ground, 30);
    expect(contacts).toHaveLength(1);
    expect(contacts[0]?.positionM.z).toBeCloseTo(figure.polar_radius_m, 3);
    // TEST HULL's farthest vertex, a corner of its 8 m × 5 m stern 10 m aft: √(4² + 2.5² + 10²).
    expect(contacts[0]?.radiusM).toBeCloseTo(Math.hypot(4, 2.5, 10), 12);
  });

  it("takes no contact of a craft at 2 km and receding, nor of a craft far from the body", () => {
    expect(sceneContacts(withCraft(2_000, 5), moon, ground, 30)).toEqual([]);
    expect(sceneContacts(SCENE, moon, ground, 30)).toEqual([]);
  });

  it("forces the pass's region under a craft descending over its survey", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, handing(5e6, {}, withCraft(900, -5)));
    h.session.step();
    await answerField(h, 1, [fieldCellIndex({ face: 2, level: FIELD_LEVEL, i: 16, j: 16 })]);
    h.session.step();
    const forced = [...(h.session.pass(MOON)?.lastStep?.selection.patches.values() ?? [])].filter(
      (p) => p.forced,
    );
    expect(forced.length).toBeGreaterThan(0);
  });
});
