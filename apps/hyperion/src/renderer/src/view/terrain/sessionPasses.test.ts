/**
 * The terrain session's passes (plan R10, R10.T16.b): one a handed-over body, shared by the views
 * that hand it over, made at its first hand-over and disposed at its field's release; the mask held
 * behind the rebakes a block names; the fault; the device loss; and the releases, over R03's
 * `FakeWebSocket`, scripted height workers and R01's counting engine. The wireframe's passes
 * (R10.T11) beside the lit ones close the file; the hand-over they take is `sessionWire.test.ts`'.
 */

import type { BodyIdHex, DetailSeedHex, UniverseIdHex } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { instrumentViewId, PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { add, normalise, scale, vec3 } from "../../geometry/vec3";
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
  moonRangeM,
  SESSION_FOV_X_RAD,
  SESSION_MOON,
  SESSION_MOON_RADIUS_M,
  SESSION_PRESENTED,
  SESSION_SCENE,
  viewAt,
} from "../../test/terrainSessionFixtures";
import { aViewCraft, FIXTURE_SYSTEM, NO_TURN } from "../../test/viewFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { lawFor } from "../appearance/phase";
import { type BodyFramePlan, type LitBodyInput, planLitBodies } from "../bodies/draw";
import { type ViewId, viewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { type MemoryCategory, textureBytes } from "../engine/memory";
import { annulusEdges, DISC_ANNULI_HIGH } from "../lighting/annuli";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import {
  type QualitySetting,
  WIREFRAME_TERRAIN_SLOTS,
  WIREFRAME_TERRAIN_TAU_PX,
} from "../quality/qualitySetting";
import { AU_M } from "../scenes/kept";
import type { ViewBody, ViewScene } from "../scene/model";
import { stepTerrainSession } from "../../displays/view/useTerrainSession";
import type { LitRegime } from "../bodies/regime";
import {
  ANNUNCIATION_ONSET_MS,
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
} from "./annunciation";
import { TerrainFailureGuard } from "./failureGuard";
import { fieldCellIndex } from "./coverage";
import { LiveFrameTerrain } from "./frameTerrain";
import { allocationTally } from "./gpu/allocationTally";
import { datumGroundUnder } from "./grounded";
import { limbDistanceM, pixelAngleRad } from "./handover";
import { childKeys, type PatchKey, patchKeyString } from "./patchKey";
import { MAX_CONTACTS } from "./gpu/uniforms";
import { CASCADE_BYTES, casterReachM } from "./shadows/cascades";
import { SHADOW_PLACEHOLDER, SHADOW_PLACEHOLDER_NAME } from "./shadows/terrainShadows";
import { PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "./priority";
import {
  sceneContacts,
  TerrainSession,
  terrainPassViewOf,
  type TerrainViewReport,
} from "./session";
import type {
  TerrainPass,
  TerrainView,
  TerrainViewFrame,
  TerrainViewFrameInput,
} from "./terrainPass";
import { PALETTE_NONE, type TerrainLayers } from "./workers/messages";
import { MAX_CONSECUTIVE_WORKER_FAILURES } from "./workers/pool";

const UNIVERSE: UniverseIdHex = "000000000000002a";
const MOON: BodyIdHex = SESSION_MOON;
const SEED: DetailSeedHex = "0123456789abcdef";
const PRIMARY = PRIMARY_VIEW_ID;
const INSTRUMENT = instrumentViewId(1);
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

async function harness(setting: QualitySetting = "high"): Promise<Harness> {
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
    setting,
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
  describe("a body's pass, made at its first hand-over", () => {
    /** The moon's field held, then handed over by the primary and an instrument for two steps. */
    async function handedOverTwice(): Promise<{
      readonly h: Harness;
      readonly first: TerrainPass | null;
    }> {
      const h = await harness();
      h.session.noteView(PRIMARY, notHanding(5e6));
      h.session.step();
      await answerField(h);
      h.session.step();
      h.session.noteView(PRIMARY, handing(5e6));
      h.session.noteView(INSTRUMENT, handing(6e6, { streamPriority: "secondary" }));
      h.session.step();
      const first = h.session.pass(MOON);
      h.session.step();
      return { h, first };
    }

    it("is not made while the body's field is held but no view hands it over", async () => {
      const h = await harness();
      h.session.noteView(PRIMARY, notHanding(5e6));
      h.session.step();
      await answerField(h);
      h.session.step();
      expect(h.session.passBodies.size).toBe(0);
    });

    it("is made for the body handed over", async () => {
      const { h } = await handedOverTwice();
      expect([...h.session.passBodies]).toEqual([MOON]);
    });

    it("is the one pass at every later step", async () => {
      const { h, first } = await handedOverTwice();
      expect(h.session.pass(MOON)).toBe(first);
    });

    it("takes every view that hands the body over", async () => {
      const { first } = await handedOverTwice();
      expect([...(first?.viewIds ?? [])]).toEqual([PRIMARY, INSTRUMENT]);
    });
  });

  describe("a pass that a device loss refused at its first hand-over", () => {
    /** The moon's field held and handed over by the primary, its first pass made during a loss. */
    async function refused(): Promise<Harness> {
      const h = await harness();
      h.session.noteView(PRIMARY, handing(5e6));
      h.session.step();
      await answerField(h);
      h.engine.loseDevice();
      h.session.step();
      return h;
    }

    it("is not made while the loss lasts", async () => {
      const h = await refused();
      expect(h.session.passBodies.size).toBe(0);
    });

    it("is made at the next step after the restore", async () => {
      const h = await refused();
      h.engine.restore();
      h.session.step();
      await settle();
      expect([...h.session.passBodies]).toEqual([MOON]);
    });
  });

  it("hands nothing over for a view whose last frame stood in with the wireframe", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, handing(5e6, { drawnStyle: "wireframe" }));
    h.session.step();
    await answerField(h);
    h.session.step();
    expect(h.session.passBodies.size).toBe(0);
  });

  it("hands a wireframe view's body to its wireframe pass alone, never the lit one (R10.T11)", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, handing(5e6, { style: "wireframe", drawnStyle: "wireframe" }));
    h.session.step();
    await answerField(h);
    h.session.step();
    expect([[...h.session.passBodies], [...h.session.wirePassBodies]]).toEqual([[], [MOON]]);
  });

  it("hands nothing over of a body whose rotation is not known", async () => {
    const h = await harness();
    const unturned = withMoon(SCENE, { ...sceneMoon(SCENE), rotation: null });
    h.session.noteView(PRIMARY, handing(5e6, {}, unturned));
    h.session.step();
    await answerField(h);
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
    // But the shadow cascades' placeholder, the session's, which it lets go at its teardown
    // (R10.T9.b).
    expect(h.engine.live).toEqual({ ...before, textures: before.textures + 1 });
    expect((["height-cache", "other"] as const).map((c) => h.tally.liveBytes(c))).toEqual([
      bytes[0],
      (bytes[1] ?? 0) + textureBytes(SHADOW_PLACEHOLDER),
    ]);
  });

  it("leaves the tallies where they started after 50 hand-overs and releases", async () => {
    const h = await harness();
    const categories: ReadonlyArray<MemoryCategory> = ["height-cache", "other", "coarse-field-gpu"];
    const before = h.engine.live;
    const bytes = categories.map((c) => h.tally.liveBytes(c));
    for (let n = 0; n < 50; n += 1) {
      h.session.noteView(PRIMARY, handing(5e6));
      h.session.step();
      // The field is asked afresh each time: answered, the body handed over. Each cycle hands
      // over and releases after the one before, so the cycles cannot run together.
      // oxlint-disable-next-line no-await-in-loop
      await answerField(h, 1);
      h.session.step();
      // The cycle's pass is made once its field has settled, before it is released.
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
    // The session's own materials, compiled at its making (R10.T14.d), go with it.
    expect(h.engine.live).toEqual({ ...before, materials: 0 });
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
  /**
   * The moon handed over by the primary and an instrument, a third view handing nothing over, the
   * changes of the session's faults counted in `told`.
   */
  async function threeViews(): Promise<{ readonly h: Harness; readonly told: { count: number } }> {
    const h = await harness();
    await handedOver(h);
    h.session.noteView(INSTRUMENT, handing(6e6, { streamPriority: "secondary" }));
    h.session.noteView(THIRD, notHanding(6e6));
    h.session.step();
    const told = { count: 0 };
    h.session.subscribeFaults(() => {
      told.count += 1;
    });
    return { h, told };
  }

  /** {@link threeViews} with every worker lost, stepped once. */
  async function faulted(): Promise<{ readonly h: Harness; readonly told: { count: number } }> {
    const views = await threeViews();
    loseEveryWorker(views.h);
    views.h.session.step();
    return views;
  }

  it("shows no fault while its workers live", async () => {
    const { h } = await threeViews();
    expect(h.session.terrainFaults.size).toBe(0);
  });

  it("shows the fault on every view that hands a body over once every worker is lost", async () => {
    const { h } = await faulted();
    expect(new Map(h.session.terrainFaults)).toEqual(
      new Map([
        [PRIMARY, TERRAIN_NOT_AVAILABLE],
        [INSTRUMENT, TERRAIN_NOT_AVAILABLE],
      ]),
    );
  });

  it("tells its listeners of the fault once", async () => {
    const { told } = await faulted();
    expect(told.count).toBe(1);
  });

  it("draws nothing under the fault, giving the fault and no annunciation", async () => {
    const { h } = await faulted();
    const frame = h.session.drawTerrain(PRIMARY, MOON, frameAt(5_000));
    expect([frame?.draw, frame?.fault, frame?.annunciation]).toEqual([
      null,
      TERRAIN_NOT_AVAILABLE,
      null,
    ]);
  });

  it("keeps the same map while nothing changes, for useSyncExternalStore", async () => {
    const { h } = await faulted();
    const faults = h.session.terrainFaults;
    h.session.step();
    expect(h.session.terrainFaults).toBe(faults);
  });

  it("tells its listeners nothing while nothing changes", async () => {
    const { h, told } = await faulted();
    h.session.step();
    expect(told.count).toBe(1);
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

  it("takes a craft at 900 m and closing as one contact", () => {
    expect(sceneContacts(withCraft(900, -5), moon, ground, 30)).toHaveLength(1);
  });

  it("puts the contact on the ground beneath the craft", () => {
    const contacts = sceneContacts(withCraft(900, -5), moon, ground, 30);
    expect(contacts[0]?.positionM.z).toBeCloseTo(figure.polar_radius_m, 3);
  });

  it("gives the contact the hull's radius", () => {
    const contacts = sceneContacts(withCraft(900, -5), moon, ground, 30);
    // TEST HULL's farthest vertex, a corner of its 8 m × 5 m stern 10 m aft: √(4² + 2.5² + 10²).
    expect(contacts[0]?.radiusM).toBeCloseTo(Math.hypot(4, 2.5, 10), 12);
  });

  it("takes no contact of a craft at 2 km and receding", () => {
    expect(sceneContacts(withCraft(2_000, 5), moon, ground, 30)).toEqual([]);
  });

  it("takes no contact of a craft far from the body", () => {
    expect(sceneContacts(SCENE, moon, ground, 30)).toEqual([]);
  });

  it("judges a craft in the system's frame by its velocity less the body's, closing on it", () => {
    // The moon moves at 1 km/s along x and 10 m/s up its pole; the craft, 900 m over the pole,
    // rises at 5 m/s with it along x: it closes on the ground at 5 m/s.
    const moving: ViewBody = {
      ...moon,
      retarded: {
        centreM: moon.centreM,
        velocityMPerS: vec3(1_000, 0, 10),
        lightTimeS: 0,
      },
    };
    const scene: ViewScene = {
      ...withMoon(SCENE, moving),
      craft: [
        aViewCraft({
          pose: {
            position: {
              kind: "system",
              system: FIXTURE_SYSTEM,
              m: add(moon.centreM, vec3(0, 0, figure.polar_radius_m + 900)),
            },
            attitude: NO_TURN,
          },
          velocityMPerS: vec3(1_000, 0, 5),
        }),
      ],
    };
    expect(sceneContacts(scene, moving, ground, 30)).toHaveLength(1);
  });

  it("takes no contact over a body whose rotation is not known", () => {
    expect(sceneContacts(withCraft(900, -5), { ...moon, rotation: null }, ground, 30)).toEqual([]);
  });

  it("takes at most MAX_CONTACTS of the craft over the body", () => {
    const one = withCraft(900, -5).craft[0] ?? fail();
    const scene: ViewScene = {
      ...SCENE,
      // The scene's own ship first, as the scene needs it among its craft.
      craft: Array.from({ length: MAX_CONTACTS + 3 }, (_, n) => ({
        ...one,
        id: n === 0 ? one.id : `craft-${String(n)}`,
      })),
    };
    expect(sceneContacts(scene, moon, ground, 30)).toHaveLength(MAX_CONTACTS);
  });

  it("passes over a craft the scene cannot place against the body, and takes the rest", () => {
    const placed = withCraft(900, -5).craft[0] ?? fail();
    const elsewhere = aViewCraft({
      id: "elsewhere",
      pose: {
        position: { kind: "system", system: "0000000000000099", m: vec3(0, 0, 0) },
        attitude: NO_TURN,
      },
      velocityMPerS: vec3(0, 0, -5),
    });
    const scene: ViewScene = { ...SCENE, craft: [elsewhere, placed] };
    expect(sceneContacts(scene, moon, ground, 30)).toHaveLength(1);
  });

  it("weighs a view by its budget's streaming priority: a secondary's a quarter of a primary's", () => {
    const weightOf = (streamPriority: "primary" | "secondary"): number | undefined =>
      terrainPassViewOf(INSTRUMENT, handing(6e6, { streamPriority }), moon)?.weight;
    expect([weightOf("primary"), weightOf("secondary")]).toEqual([
      PRIMARY_VIEW_WEIGHT,
      SECONDARY_VIEW_WEIGHT,
    ]);
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

/** A 1080p camera across 60°, looking along −z, as `viewAt`'s views do. */
const FRAME_CAMERA = { orientation: NO_TURN, fovXRad: SESSION_FOV_X_RAD };

/** The moon 5,000 km ahead of a view, handed over in its plan, and the plan, under a Sun. */
function moonPlan(): { readonly plan: BodyFramePlan; readonly body: LitBodyInput } {
  const a = SESSION_MOON_RADIUS_M;
  const distanceM = 5e6;
  const body: LitBodyInput = {
    id: MOON,
    centreM: vec3(0, 0, -distanceM),
    figure: { equatorialRadiusM: a, polarRadiusM: a, pole: vec3(0, 0, 1) },
    photometry: PROVISIONAL_PHOTOMETRY,
    rotation: IDENTITY_ROTATION,
    handoverReliefM:
      4 * limbDistanceM(distanceM, a) * pixelAngleRad(FRAME_CAMERA, SESSION_PRESENTED),
    lighting: undefined,
  };
  const plan = planLitBodies(
    [body],
    [
      {
        disc: sunLikeHostDisc(),
        centreM: add(body.centreM, scale(normalise(vec3(0.3, 0.2, 1)), AU_M)),
      },
    ],
    {
      camera: FRAME_CAMERA,
      viewport: SESSION_PRESENTED,
      exposureScale: 1e-5,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
    },
    new Map(),
  );
  if (plan.meshes[0]?.role.kind !== "handed-over") {
    throw new Error("the moon was not handed over");
  }
  return { plan, body };
}

/** View `view`'s frame terrain of the session's passes. */
function frameTerrainOf(h: Harness, view: ViewId, nowMs = 0): LiveFrameTerrain {
  return new LiveFrameTerrain(h.session, {
    view,
    camera: FRAME_CAMERA,
    presentedPx: SESSION_PRESENTED,
    exposureScale: 1e-5,
    occluderSlopePx: 3,
    nowMs,
  });
}

/** Steps the session and answers the bakes asked, `rounds` times. */
async function streamBakes(h: Harness, rounds: number): Promise<void> {
  for (let n = 0; n < rounds; n += 1) {
    h.session.step();
    answerBakes(h, new Set([0, 1]), new Map());
    // Each round answers the bakes the round before asked, so the rounds cannot run together.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
  h.session.step();
}

describe("each view's frame terrain through the session (R10.T16.c)", () => {
  it("draws a body's terrain in the view that hands it over, and in another only once it hands it over too", async () => {
    const h = await harness();
    await handedOver(h);
    h.session.noteView(INSTRUMENT, notHanding(5e6));
    await streamBakes(h, 4);
    const { plan, body } = moonPlan();
    const primary = frameTerrainOf(h, PRIMARY).draws(plan, [body]);
    const instrumentBefore = frameTerrainOf(h, INSTRUMENT).draws(plan, [body]);
    h.session.noteView(INSTRUMENT, handing(5e6, { streamPriority: "secondary" }));
    await streamBakes(h, 2);
    const instrumentAfter = frameTerrainOf(h, INSTRUMENT, 16).draws(plan, [body]);
    expect([primary.length, instrumentBefore.length, instrumentAfter.length]).toEqual([1, 0, 1]);
    // One pass draws both, each view through its own buffers.
    expect(instrumentAfter[0]?.storageBuffers?.["instances"]?.name).toBe(
      `terrain instances (${INSTRUMENT})`,
    );
  });

  it("gives the body's mask to a view only once its draw set of the body has nothing missing", async () => {
    const h = await harness();
    await handedOver(h);
    const beforeBakes = frameTerrainOf(h, PRIMARY).surveyCoverage(MOON);
    await streamBakes(h, 4);
    const afterBakes = frameTerrainOf(h, PRIMARY).surveyCoverage(MOON);
    expect(h.session.coverageMask(MOON)).not.toBeNull();
    expect([beforeBakes, afterBakes]).toEqual([null, h.session.coverageMask(MOON)]);
  });

  it("gives the terrain's depth for the symbology of each body it draws", async () => {
    const h = await harness();
    await handedOver(h);
    await streamBakes(h, 4);
    const { plan, body } = moonPlan();
    const terrain = frameTerrainOf(h, PRIMARY);
    terrain.draws(plan, [body]);
    expect(terrain.symbologyDepth().map((draw) => draw.material.name)).toEqual([
      h.session.pass(MOON)?.symbologyDepthMaterial?.name,
    ]);
  });
});

/** The session as the view's loop steps it, each step throwing a bug while `failing.on` holds. */
function steppedAs(
  h: Harness,
  failing: { on: boolean },
): Pick<TerrainSession, "step" | "failComputation"> {
  return {
    step: () => {
      if (failing.on) {
        throw new Error("a bug of the session's step");
      }
      h.session.step();
    },
    failComputation: () => {
      h.session.failComputation();
    },
  };
}

/** Steps `stepped` through `guard` at each animation frame, 16 ms apart, from `fromMs` to `toMs`. */
function loop(
  stepped: Pick<TerrainSession, "step" | "failComputation">,
  guard: TerrainFailureGuard,
  fromMs: number,
  toMs: number,
): void {
  for (let atMs = fromMs; atMs <= toMs; atMs += 16) {
    stepTerrainSession(stepped, guard, atMs);
  }
}

/** Every bake the workers have been asked for. */
function bakesAsked(h: Harness): number {
  return h.workers.reduce((n, w) => n + w.received.filter((m) => m.kind === "bake").length, 0);
}

describe("the terrain session's computation fault (R10.T16.e; decision-r10-t16-leans §8)", () => {
  /**
   * The moon handed over by the primary and an instrument, its bakes streamed so that the primary
   * draws it and is given its mask, and a third view handing nothing over.
   */
  async function streaming(): Promise<Harness> {
    const h = await harness();
    await handedOver(h);
    h.session.noteView(INSTRUMENT, handing(6e6, { streamPriority: "secondary" }));
    h.session.noteView(THIRD, notHanding(6e6));
    await streamBakes(h, 4);
    return h;
  }

  it("raises the fault on each view that hands a body over once the step has thrown for 250 ms", async () => {
    const h = await streaming();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const stepped = steppedAs(h, { on: true });
    const guard = new TerrainFailureGuard();
    loop(stepped, guard, 0, ANNUNCIATION_ONSET_MS - 1);
    const before = h.session.terrainFaults.size;
    loop(stepped, guard, ANNUNCIATION_ONSET_MS, ANNUNCIATION_ONSET_MS);
    expect([before, new Map(h.session.terrainFaults)]).toEqual([
      0,
      new Map([
        [PRIMARY, TERRAIN_COMPUTATION_FAILED],
        [INSTRUMENT, TERRAIN_COMPUTATION_FAILED],
      ]),
    ]);
  });

  it("raises nothing for a single failed step", async () => {
    const h = await streaming();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const failing = { on: true };
    const stepped = steppedAs(h, failing);
    const guard = new TerrainFailureGuard();
    loop(stepped, guard, 0, 0);
    failing.on = false;
    loop(stepped, guard, 16, 1_000);
    expect([h.session.terrainFaults.size, h.session.pass(MOON)?.faulted]).toEqual([0, false]);
  });

  it("stops the terrain's demand: no bake is asked as the view closes in", async () => {
    const h = await harness();
    // Four cells about the north pole of face +z, under the primary 3,000 km out.
    const cells = [
      { i: 15, j: 15 },
      { i: 16, j: 15 },
      { i: 15, j: 16 },
      { i: 16, j: 16 },
    ].map(({ i, j }) => fieldCellIndex({ face: 2, level: FIELD_LEVEL, i, j }));
    h.session.noteView(PRIMARY, handing(3e6));
    h.session.step();
    await answerField(h, 1, cells);
    h.session.step();
    await settle();
    for (const worker of h.workers) {
      answerBlock(worker, 0, blockChange(FIELD_LEVEL, runsOf(cells)));
    }
    await streamBakes(h, 2);
    h.session.failComputation();
    const asked = bakesAsked(h);
    // At 2,200 km a pass still stepping asks for finer patches over the survey (8 more bakes).
    h.session.noteView(PRIMARY, handing(2.2e6));
    await streamBakes(h, 3);
    expect(bakesAsked(h) - asked).toBe(0);
  });

  it("stops the terrain's draw: none in any view, with no annunciation, and the mask withheld", async () => {
    const h = await streaming();
    const maskBefore = frameTerrainOf(h, PRIMARY).surveyCoverage(MOON);
    h.session.failComputation();
    h.session.step();
    const { plan, body } = moonPlan();
    const terrain = frameTerrainOf(h, PRIMARY, 16);
    const draws = terrain.draws(plan, [body]);
    const instrument = h.session.drawTerrain(INSTRUMENT, MOON, frameAt(5_000));
    expect(maskBefore).not.toBeNull();
    expect([
      draws.length,
      terrain.annunciation,
      terrain.surveyCoverage(MOON),
      instrument?.draw,
      instrument?.fault,
      instrument?.annunciation,
      instrument?.maskReady,
    ]).toEqual([0, null, null, null, TERRAIN_COMPUTATION_FAILED, null, false]);
  });

  it("makes no pass once faulted: a body handed over after it keeps its figure, its mask withheld", async () => {
    const h = await harness();
    h.session.noteView(PRIMARY, handing(5e6));
    h.session.step();
    // The field held, the moon's pass would be made at the next step.
    await answerField(h);
    h.session.failComputation();
    h.session.step();
    await settle();
    expect([
      h.session.pass(MOON),
      h.session.coverageMask(MOON) === null,
      frameTerrainOf(h, PRIMARY).surveyCoverage(MOON),
      new Map(h.session.terrainFaults),
    ]).toEqual([null, false, null, new Map([[PRIMARY, TERRAIN_COMPUTATION_FAILED]])]);
  });

  it("is sticky: the fault stands, and the passes stay halted, however the steps go after it", async () => {
    const h = await streaming();
    h.session.failComputation();
    const faults = h.session.terrainFaults;
    await streamBakes(h, 2);
    expect([h.session.terrainFaults, h.session.pass(MOON)?.faulted]).toEqual([faults, true]);
  });

  it("keeps the first cause raised: every height worker lost before the computation's fault", async () => {
    const h = await streaming();
    loseEveryWorker(h);
    h.session.step();
    h.session.failComputation();
    h.session.step();
    expect(new Set(h.session.terrainFaults.values())).toEqual(new Set([TERRAIN_NOT_AVAILABLE]));
  });

  it("shows the fault on every view reported where the views handing a body over cannot be told", async () => {
    const h = await streaming();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    /** A view's regimes that cannot be read: a bug the step meets as it finds the views handing over. */
    class UnreadableRegimes extends Map<BodyIdHex, LitRegime> {
      override [Symbol.iterator](): never {
        throw new Error("a bug of the view's regimes");
      }
    }
    h.session.noteView(THIRD, { ...notHanding(6e6), regimes: new UnreadableRegimes() });
    const stepped = steppedAs(h, { on: false });
    loop(stepped, new TerrainFailureGuard(), 0, ANNUNCIATION_ONSET_MS + 16);
    expect(new Map(h.session.terrainFaults)).toEqual(
      new Map([
        [PRIMARY, TERRAIN_COMPUTATION_FAILED],
        [INSTRUMENT, TERRAIN_COMPUTATION_FAILED],
        [THIRD, TERRAIN_COMPUTATION_FAILED],
      ]),
    );
  });
});

/** A wireframe view `distanceM` above the moon's north pole (R10.T11). */
function wireAt(distanceM: number, overrides: Partial<TerrainViewReport> = {}): TerrainViewReport {
  return { ...viewAt(distanceM, "wireframe", { scene: SCENE }), ...overrides };
}

/** The moon handed over by `views`' reports, its field answered over `cells`, its passes made. */
async function handedOverBy(
  h: Harness,
  views: ReadonlyArray<readonly [ViewId, TerrainViewReport]>,
  cells: ReadonlyArray<number> = [0, 1],
): Promise<void> {
  for (const [id, report] of views) {
    h.session.noteView(id, report);
  }
  h.session.step();
  await answerField(h, 1, cells);
  h.session.step();
  await settle();
}

/** The survey each pass's bakes are answered over, by its weights, or `null` to hold them. */
interface BakeSurveys {
  /** The lit pass's (`weights: "baked"`). */
  readonly baked: ReadonlySet<number> | null;
  /** The wireframe pass's (`weights: "skipped"`). */
  readonly skipped: ReadonlySet<number> | null;
}

/** Answers every bake the workers hold of a pass whose survey `surveys` gives; returns how many. */
function answerBakesOf(h: Harness, surveys: BakeSurveys): number {
  let answered = 0;
  for (const worker of h.workers) {
    for (const request of worker.pendingBakes) {
      const { key, settings } = request;
      const source = settings.source;
      const surveyed = source.kind === "body" ? surveys[source.weights] : null;
      if (surveyed === null) {
        continue;
      }
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
      answered += 1;
    }
  }
  return answered;
}

/** Steps the session and answers the bakes asked over `surveys`, `rounds` times. */
async function streamBakesOf(h: Harness, rounds: number, surveys: BakeSurveys): Promise<void> {
  for (let n = 0; n < rounds; n += 1) {
    h.session.step();
    answerBakesOf(h, surveys);
    // Each round answers the bakes the round before asked, so the rounds cannot run together.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
  h.session.step();
}

/** Both passes' bakes answered over cells 0 and 1. */
const BOTH_OVER_FIRST_CELLS: BakeSurveys = { baked: new Set([0, 1]), skipped: new Set([0, 1]) };

/** A wireframe frame's input from the primary's camera, its contours at 200 m (R10.T11). */
function wireFrameAt(nowMs: number): TerrainViewFrameInput {
  return {
    view: PRIMARY_VIEW,
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [0, 0, 0],
    exposureScale: 1,
    wire: {
      intervalM: 200,
      seaLevelM: null,
      strokeScale: 1,
      graticule: null,
      colours: {
        muted: new Float32Array([0.3, 0.3, 0.3, 1]),
        text: new Float32Array([0.8, 0.8, 0.8, 1]),
        casing: new Float32Array([0, 0, 0, 1]),
      },
      occluderSlopePx: 3,
    },
    nowMs,
  };
}

/** The material a view's frame of a pass draws with, or `null` where it draws nothing. */
function material(frame: TerrainViewFrame | null): string | null {
  return frame?.draw?.material.name ?? null;
}

/** Cells of face +z at the field's level, by their `[i, j]`. */
function faceZCells(ijs: ReadonlyArray<readonly [number, number]>): number[] {
  return ijs.map(([i, j]) => fieldCellIndex({ face: 2, level: FIELD_LEVEL, i, j }));
}

describe("the terrain session's wireframe pass (R10.T11; decision D10)", () => {
  it("is made at a wireframe view's first hand-over, its cache of the wireframe's 520 slots", async () => {
    const h = await harness();
    await handedOverBy(h, [[PRIMARY, wireAt(5e6)]]);
    expect(h.session.wirePass(MOON)?.layout.slots.slotCount).toBe(WIREFRAME_TERRAIN_SLOTS);
  });

  it("asks its bakes without the class weights and the horizon map", async () => {
    const h = await harness();
    await handedOverBy(h, [[PRIMARY, wireAt(5e6)]]);
    const weights = h.workers.flatMap((w) =>
      w.pendingBakes.map((b) =>
        b.settings.source.kind === "body" ? b.settings.source.weights : null,
      ),
    );
    expect([weights.length > 0, new Set(weights)]).toEqual([true, new Set(["skipped"])]);
  });

  it("bakes through a lane of the session's one pool, starting no worker of its own", async () => {
    const h = await harness();
    await handedOverBy(h, [[PRIMARY, wireAt(5e6)]]);
    expect([h.workers.length, h.workers.every((w) => w.pendingBakes.length > 0)]).toEqual([
      2,
      true,
    ]);
  });

  it("hands a photorealistic view's body to its lit pass alone, never the wireframe's", async () => {
    const h = await harness();
    await handedOverBy(h, [[PRIMARY, handing(5e6)]]);
    expect([[...h.session.passBodies], [...h.session.wirePassBodies]]).toEqual([[MOON], []]);
  });

  it("makes no wireframe pass of a body with no survey, whose relief hands nothing over", async () => {
    const h = await harness();
    await handedOverBy(h, [[PRIMARY, wireAt(5e6)]], []);
    expect(h.session.wirePassBodies.size).toBe(0);
  });
});

describe("a body handed over in both styles at once (R10.T11)", () => {
  /** The views of both styles by the moon's pole, the primary photorealistic. */
  const BOTH: ReadonlyArray<readonly [ViewId, TerrainViewReport]> = [
    [PRIMARY, handing(5e6)],
    [INSTRUMENT, wireAt(5e6, { streamPriority: "secondary" })],
  ];

  /** Both views far beyond twice their ranges, where the field is released. */
  function farAway(h: Harness): void {
    h.session.noteView(PRIMARY, notHanding(7e7));
    h.session.noteView(INSTRUMENT, wireAt(7e7, { streamPriority: "secondary" }));
    h.session.step();
  }

  it("has both passes, on the one pool's two workers", async () => {
    const h = await harness();
    await handedOverBy(h, BOTH);
    expect([[...h.session.passBodies], [...h.session.wirePassBodies], h.workers.length]).toEqual([
      [MOON],
      [MOON],
      2,
    ]);
  });

  it("takes each view into its own style's pass", async () => {
    const h = await harness();
    await handedOverBy(h, BOTH);
    expect([
      [...(h.session.pass(MOON)?.viewIds ?? [])],
      [...(h.session.wirePass(MOON)?.viewIds ?? [])],
    ]).toEqual([[PRIMARY], [INSTRUMENT]]);
  });

  it("draws each view through its own style's pass, and neither through the other's", async () => {
    const h = await harness();
    await handedOverBy(h, BOTH);
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    expect([
      material(h.session.drawTerrain(PRIMARY, MOON, frameAt(0)))?.startsWith("terrain-lit-"),
      material(h.session.drawWireTerrain(INSTRUMENT, MOON, wireFrameAt(0)))?.startsWith(
        "terrain-wire-",
      ),
      material(h.session.drawTerrain(INSTRUMENT, MOON, frameAt(16))),
      material(h.session.drawWireTerrain(PRIMARY, MOON, wireFrameAt(16))),
    ]).toEqual([true, true, null, null]);
  });

  it("gives the mask a block's cells only once the wireframe pass's rebakes have landed too", async () => {
    const h = await harness();
    // The first survey: four cells about the north pole of face +z; the delta, two beside them.
    const first = faceZCells([
      [15, 15],
      [16, 15],
      [15, 16],
      [16, 16],
    ]);
    const second = faceZCells([
      [17, 15],
      [17, 16],
    ]);
    const both = new Set([...first, ...second]);
    await handedOverBy(h, BOTH, first);
    for (const worker of h.workers) {
      answerBlock(worker, 0, blockChange(FIELD_LEVEL, runsOf(first)));
    }
    // The lit pass's bakes hold the delta's cells already, so that its block names no rebake of
    // the lit pass's, and the wireframe pass's do not, so that it names the wireframe pass's: under
    // a countdown of the lit pass's alone, the mask would take the cells at once.
    await streamBakesOf(h, 6, { baked: both, skipped: new Set(first) });
    const lit = h.session.pass(MOON);
    const wire = h.session.wirePass(MOON);
    if ((lit?.lastStep?.drawSet.count ?? 0) === 0 || (wire?.lastStep?.drawSet.count ?? 0) === 0) {
      throw new Error("both passes draw patches over the survey");
    }
    h.session.noteSurfaceRevisions([{ body: MOON, revision: 2 }]);
    await answerField(h, 2, second);
    for (const worker of h.workers) {
      answerBlock(worker, 1, blockChange(FIELD_LEVEL, runsOf(second)));
    }
    const held = {
      lit: lit?.settled,
      wire: wire?.settled,
      masked: second.some((cell) => maskCells(h).has(cell)),
    };
    await streamBakesOf(h, 6, { baked: both, skipped: both });
    const landed = {
      lit: lit?.settled,
      wire: wire?.settled,
      masked: second.every((cell) => maskCells(h).has(cell)),
    };
    expect([held, landed]).toEqual([
      { lit: true, wire: false, masked: false },
      { lit: true, wire: true, masked: true },
    ]);
  });

  it("disposes of both passes at the field's release, its handles and tallies where they started", async () => {
    const h = await harness();
    const categories: ReadonlyArray<MemoryCategory> = ["height-cache", "other", "coarse-field-gpu"];
    const before = h.engine.live;
    const bytes = categories.map((c) => h.tally.liveBytes(c));
    await handedOverBy(h, BOTH);
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    h.session.drawTerrain(PRIMARY, MOON, frameAt(0));
    h.session.drawWireTerrain(INSTRUMENT, MOON, wireFrameAt(0));
    farAway(h);
    expect([h.session.passBodies.size, h.session.wirePassBodies.size]).toEqual([0, 0]);
    // But the session's shadow placeholder, let go at its teardown (R10.T9.b).
    expect(h.engine.live).toEqual({ ...before, textures: before.textures + 1 });
    expect(categories.map((c) => h.tally.liveBytes(c))).toEqual([
      bytes[0],
      (bytes[1] ?? 0) + textureBytes(SHADOW_PLACEHOLDER),
      bytes[2],
    ]);
  });

  it("draws nothing of the wireframe pass once the field is released", async () => {
    const h = await harness();
    await handedOverBy(h, BOTH);
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    farAway(h);
    expect(h.session.drawWireTerrain(INSTRUMENT, MOON, wireFrameAt(16))).toBeNull();
  });

  it("disposes of both passes when disposed, leaving no handle", async () => {
    const h = await harness();
    const before = h.engine.live;
    await handedOverBy(h, BOTH);
    h.session.dispose();
    // The session's own materials, compiled at its making (R10.T14.d), go with it.
    expect([h.engine.live, h.session.passBodies.size, h.session.wirePassBodies.size]).toEqual([
      { ...before, materials: 0 },
      0,
      0,
    ]);
  });
});

/** The materials asked of the engine from the `from`-th, by name (R10.T14.d). */
function compiled(h: Harness, from = 0): string[] {
  return h.engine.compiles.slice(from).map(({ name }) => name);
}

/** What each pass of the moon draws with: the lit, its depth and the wireframe's, by name. */
function drawnWith(h: Harness): ReadonlyArray<string | null> {
  return [
    material(h.session.drawTerrain(PRIMARY, MOON, frameAt(16))),
    h.session.pass(MOON)?.symbologyDepthMaterial?.name ?? null,
    material(h.session.drawWireTerrain(INSTRUMENT, MOON, wireFrameAt(16))),
  ];
}

describe("the terrain session's materials (R10.T14.d; decision-r10-t14a §1)", () => {
  /** The views of both styles by the moon's pole, the primary photorealistic. */
  const BOTH: ReadonlyArray<readonly [ViewId, TerrainViewReport]> = [
    [PRIMARY, handing(5e6)],
    [INSTRUMENT, wireAt(5e6, { streamPriority: "secondary" })],
  ];

  /** The high setting's four, its shadow cascades' caster among them (R10.T9.b). */
  const HIGH = [
    "terrain-lit-baked-offsets",
    "terrain-depth-baked-offsets",
    "terrain-shadow-baked-offsets",
    "terrain-wire-baked-offsets",
  ];

  /** What the moon's passes draw with on the high setting ({@link drawnWith}). */
  const HIGH_DRAWN = [
    "terrain-lit-baked-offsets",
    "terrain-depth-baked-offsets",
    "terrain-wire-baked-offsets",
  ];

  it("compiles its setting's lit, depth, caster and wireframe materials as it is made", async () => {
    const h = await harness();
    expect([await h.session.compiled, compiled(h)]).toEqual([4, HIGH]);
  });

  it("compiles the low setting's on face differences", async () => {
    const h = await harness("low");
    await h.session.compiled;
    expect(compiled(h)).toEqual([
      "terrain-lit-face-differences",
      "terrain-depth-face-differences",
      "terrain-wire-face-differences",
    ]);
  });

  it("compiles nothing at a hand-over in either style, its passes drawing its materials", async () => {
    const h = await harness();
    await h.session.compiled;
    await handedOverBy(h, BOTH);
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    expect([compiled(h, 4), drawnWith(h), h.engine.live.materials]).toEqual([[], HIGH_DRAWN, 4]);
  });

  it("compiles nothing at a hand-over on the low setting either", async () => {
    const h = await harness("low");
    await h.session.compiled;
    await handedOverBy(h, [
      [PRIMARY, handing(5e6, { setting: "low" })],
      [INSTRUMENT, wireAt(5e6, { streamPriority: "secondary", setting: "low" })],
    ]);
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    expect([compiled(h, 3), drawnWith(h), h.engine.live.materials]).toEqual([
      [],
      [
        "terrain-lit-face-differences",
        "terrain-depth-face-differences",
        "terrain-wire-face-differences",
      ],
      3,
    ]);
  });

  it("compiles its materials once again after a device loss, and its passes none", async () => {
    const h = await harness();
    await h.session.compiled;
    await handedOverBy(h, BOTH);
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    h.engine.loseDevice();
    h.engine.restore();
    await settle();
    await streamBakesOf(h, 4, BOTH_OVER_FIRST_CELLS);
    expect([compiled(h, 4), drawnWith(h), h.engine.live.materials]).toEqual([HIGH, HIGH_DRAWN, 4]);
  });

  it("releases its materials only at its teardown, not with a pass", async () => {
    const h = await harness();
    await h.session.compiled;
    await handedOverBy(h, BOTH);
    h.session.noteView(PRIMARY, notHanding(7e7));
    h.session.noteView(INSTRUMENT, wireAt(7e7, { streamPriority: "secondary" }));
    h.session.step();
    const kept = [h.session.passBodies.size, h.engine.live.materials];
    h.session.dispose();
    expect([kept, h.engine.live.materials]).toEqual([[0, 4], 0]);
  });
});

describe("the terrain session's shadow cascades (R10.T9.b; decision-r10-t9b-design D2 and D9)", () => {
  /** The moon's figure, as its field header gives it. */
  const HEADER = moonLikeHeader(MOON);

  /**
   * A frame of the primary 30 m over the moon's north pole, looking straight down, under one star
   * 17° up, its body taking the cascades: near enough the ground that w > 0.
   */
  function nearGround(nowMs: number): TerrainViewFrameInput {
    const a = HEADER.equatorial_radius_m;
    const annuli = annulusEdges(0.6, 0.2, 2);
    const lighting = {
      lights: [
        {
          direction: normalise(vec3(1, 0, 0.3)),
          distance: AU_M / a,
          radius: 6.957e8 / a,
          illuminance: [1.3e5, 1.25e5, 1.2e5] as const,
          annuli: [annuli, annuli, annuli] as const,
        },
      ],
      occluders: [],
      secondaries: [],
      elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      exposureScale: 1e-5,
    };
    return {
      ...frameAt(nowMs),
      view: { ...PRIMARY_VIEW, cameraM: vec3(0, 0, HEADER.polar_radius_m + 30) },
      lighting,
      takesCascades: true,
    };
  }

  /** A session of `setting` with the moon handed over and its first bakes drawn. */
  async function drawing(setting: QualitySetting): Promise<Harness> {
    const h = await harness(setting);
    await handedOverBy(h, [[PRIMARY, handing(5e6, { setting })]]);
    answerBakes(h, new Set([0, 1]), new Map());
    h.session.step();
    return h;
  }

  it("makes the engine's one set at the first frame that draws them on the high setting, counts it, and lets it go at its teardown", async () => {
    const h = await drawing("high");
    const destroyed: string[] = [];
    h.engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    // A frame whose body takes none binds the placeholder first.
    h.session.drawTerrain(PRIMARY, MOON, { ...nearGround(0), takesCascades: false });
    const first = h.session.drawTerrain(PRIMARY, MOON, nearGround(16));
    h.session.drawTerrain(PRIMARY, MOON, nearGround(32));
    const living = [
      h.tally.liveBytes("shadows"),
      h.engine.counts.depthLayerTargets,
      h.session.memory().gpuBytes.get("shadows"),
    ];
    h.session.dispose();
    expect([
      (first?.cascades?.set.weight ?? 0) > 0,
      first?.cascades?.passes.length,
      living,
      h.tally.liveBytes("shadows"),
      destroyed.filter((name) => name.startsWith("terrain shadow")).toSorted(),
    ]).toEqual([
      true,
      4,
      [CASCADE_BYTES, 1, CASCADE_BYTES],
      0,
      ["terrain shadow cascades", SHADOW_PLACEHOLDER_NAME],
    ]);
  });

  it("never makes one on the low setting, its draws binding the placeholder", async () => {
    const h = await drawing("low");
    const frame = h.session.drawTerrain(PRIMARY, MOON, nearGround(0));
    expect([
      frame?.cascades ?? null,
      frame?.draw?.textures["shadowCascades"]?.name,
      h.engine.counts.depthLayerTargets,
      h.session.memory().gpuBytes.get("shadows"),
    ]).toEqual([null, SHADOW_PLACEHOLDER_NAME, 0, undefined]);
  });

  it("gives its passes the body's realised relief for the casters' reach", async () => {
    const h = await drawing("high");
    const set = h.session.drawTerrain(PRIMARY, MOON, nearGround(0))?.cascades?.set;
    if (set === undefined) {
      throw new Error("the frame drew no cascades");
    }
    const radiusM = (2 * HEADER.equatorial_radius_m + HEADER.polar_radius_m) / 3;
    expect(set.casterReachM).toBeCloseTo(
      casterReachM(set.sunElevationRad, HEADER.realised_relief_m, radiusM),
      6,
    );
  });

  it("draws the casters its registry holds into every pass of every view, until one is removed", async () => {
    const h = await drawing("high");
    const asked: number[] = [];
    const remove = h.session.shadowCasters.add((_set, index) => {
      asked.push(index);
      return [];
    });
    h.session.drawTerrain(PRIMARY, MOON, nearGround(0));
    remove();
    h.session.drawTerrain(PRIMARY, MOON, nearGround(16));
    expect(asked).toEqual([0, 1, 2, 3]);
  });
});

describe("the terrain session's fault in the wireframe (R10.T11; decision-r10-t16-contracts §7)", () => {
  it("shows the fault on a wireframe view that hands a body over once every worker is lost", async () => {
    const h = await harness();
    await handedOverBy(h, [
      [PRIMARY, wireAt(5e6)],
      [THIRD, wireAt(6e7, { streamPriority: "secondary" })],
    ]);
    loseEveryWorker(h);
    h.session.step();
    expect([...h.session.terrainFaults]).toEqual([[PRIMARY, TERRAIN_NOT_AVAILABLE]]);
  });

  it("shows the fault on the views of both styles that hand a body over", async () => {
    const h = await harness();
    await handedOverBy(h, [
      [PRIMARY, handing(5e6)],
      [INSTRUMENT, wireAt(5e6, { streamPriority: "secondary" })],
    ]);
    loseEveryWorker(h);
    h.session.step();
    expect([...h.session.terrainFaults.keys()].toSorted()).toEqual(
      [INSTRUMENT, PRIMARY].toSorted(),
    );
  });
});

describe("a wireframe view's hysteresis in the session (R10.T11)", () => {
  /** The moon's hand-over range in the wireframe, at its 4 px, m. */
  const wireRangeM = moonRangeM(WIREFRAME_TERRAIN_TAU_PX);

  /** A wireframe primary whose camera is `limbM` from the moon's limb. */
  function wireAtLimb(limbM: number): TerrainViewReport {
    return wireAt(Math.hypot(limbM, SESSION_MOON_RADIUS_M));
  }

  /** The moon handed over by the wireframe primary well inside its range, its pass made. */
  async function handedOverInside(): Promise<Harness> {
    const h = await harness();
    await handedOverBy(h, [[PRIMARY, wireAtLimb(0.5 * wireRangeM)]]);
    return h;
  }

  it("keeps a body handed over while the view moves out between the bands", async () => {
    const h = await handedOverInside();
    // At the range itself: past range ÷ 1.1, where it would not hand over afresh, and inside
    // range ÷ 0.9, where it keeps the hand-over it had.
    h.session.noteView(PRIMARY, wireAtLimb(wireRangeM));
    h.session.step();
    expect([...(h.session.wirePass(MOON)?.viewIds ?? [])]).toEqual([PRIMARY]);
  });

  it("judges a forgotten view afresh: reported again between the bands, it hands nothing over", async () => {
    const h = await handedOverInside();
    h.session.forgetView(PRIMARY);
    h.session.noteView(PRIMARY, wireAtLimb(wireRangeM));
    h.session.step();
    expect([...(h.session.wirePass(MOON)?.viewIds ?? [])]).toEqual([]);
  });
});
