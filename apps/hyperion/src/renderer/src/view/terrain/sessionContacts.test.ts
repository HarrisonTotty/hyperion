/**
 * The terrain session's contacts on a generated world (plan R10, R10.T14.c;
 * decision-r10-t14c-leans §2 and §5): the scene's craft judged on the drawn ground of a pass's
 * resident bakes (`heldGroundUnder`), as the session's own pass steps them, converging on the
 * finest bake as it lands and the datum's bit for bit where none is surveyed; `TERRAIN TEST`'s
 * script still free of contacts; a craft held in another body's frame chained through each body's
 * velocity at the epoch it is drawn at (`craftVelocityAgainst`); and the session's level tables and
 * lanes for a measurement, over R03's `FakeWebSocket` and scripted height workers.
 */

import type { BodyIdHex, DetailSeedHex, UniverseIdHex } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
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
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import {
  SESSION_MOON,
  SESSION_PLANET,
  SESSION_SCENE,
  viewAt,
} from "../../test/terrainSessionFixtures";
import {
  aBody,
  aViewCraft,
  aViewScene,
  FIXTURE_MOON,
  FIXTURE_PLANET,
  FIXTURE_SYSTEM,
  NO_TURN,
} from "../../test/viewFixtures";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import type { ViewPosition } from "../coords/position";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";
import type { RetardedCentre, ViewBody, ViewCraft, ViewScene } from "../scene/model";
import { AU_M } from "../scenes/kept";
import { blockBytes, type TerrainGolden } from "../scenes/terrainGolden";
import { TERRAIN_TEST_SCRIPT, terrainScene, terrainTestGolden } from "../scenes/terrainScene";
import { fieldCellIndex } from "./coverage";
import { vertexDir } from "./cube";
import {
  craftContact,
  datumGroundUnder,
  finestPatchSizeM,
  type GroundContact,
  heldGroundUnder,
} from "./grounded";
import { type PatchKey, patchKeyString } from "./patchKey";
import { type BodyFigure, levelBoundM, planetGeometry, surfacePoint } from "./planet";
import { ResidentHeights } from "./residentHeights";
import {
  type HeightRangeLookup,
  type Selection,
  selectPatches,
  type ViewSelectionInput,
} from "./select";
import { craftVelocityAgainst, sceneContacts, TerrainSession } from "./session";
import {
  type BakedPatch,
  type BakeSettings,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

const UNIVERSE: UniverseIdHex = "000000000000002a";
const MOON: BodyIdHex = SESSION_MOON;
const PLANET: BodyIdHex = SESSION_PLANET;
const SEED: DetailSeedHex = "0123456789abcdef";
const PRIMARY = PRIMARY_VIEW_ID;

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
  readonly workers: ScriptedHeightWorker[];
  readonly session: TerrainSession;
}

/** A session of two scripted workers over the fake socket, as `session.test.ts`' harness makes. */
async function harness(): Promise<Harness> {
  const hook = renderHook(() => useServerLink(), { wrapper: ServerLinkHarness });
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  const engine = await countingRenderEngine();
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
    setting: "high",
  });
  return {
    socket: () => FakeWebSocket.latest(),
    link: () => hook.result.current,
    workers,
    session,
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

/** Answers the latest field request with one block, at revision 1, cells 0 to 2 surveyed. */
async function answerField(h: Harness): Promise<void> {
  const { id } = latestFieldAsk(h.socket());
  const bulk = sendBlocks(h);
  h.socket().serverResponds(id, readyField(moonLikeHeader(MOON), 1, fieldCover([0, 2, 30]), bulk));
  await settle();
}

/** The moon wanted as the primary's local body, its field answered at revision 1. */
async function holdMoon(h: Harness): Promise<void> {
  h.session.noteView(PRIMARY, viewAt(5e6));
  h.session.step();
  await answerField(h);
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.useRealTimers();
});

/** A block's answer whose level table is 0, 1, … 99, told apart from another's by `offset`. */
function blockWithLevels(offset: number): ReturnType<typeof blockChange> {
  return {
    ...blockChange(5, [0, 2]),
    levels: Float64Array.from({ length: 25 * 4 }, (_, n) => n + offset),
  };
}

describe("the terrain session's level tables and lanes for a measurement (R10.T14.c)", () => {
  /** A lit lane's settings over the moon. */
  const LANE: BakeSettings = {
    vertexPath: "face-differences",
    normals: "mesh",
    source: { kind: "body", body: MOON, detailSeed: SEED, weights: "baked" },
  };

  /** The moon wanted as the primary's local body, its field answered in two blocks. */
  async function holdMoonInTwoBlocks(h: Harness): Promise<void> {
    h.session.noteView(PRIMARY, viewAt(5e6));
    h.session.step();
    const { id } = latestFieldAsk(h.socket());
    const bulk = sendBlocks(h, [fieldBlock(50, 1), fieldBlock(50, 2)]);
    h.socket().serverResponds(
      id,
      readyField(moonLikeHeader(MOON), 1, fieldCover([0, 2, 30]), bulk),
    );
    await settle();
  }

  it("gives no level table before any block's report, nor of a body not wanted", async () => {
    const h = await harness();
    await holdMoonInTwoBlocks(h);
    expect([h.session.levels(MOON), h.session.levels(PLANET)]).toEqual([null, null]);
  });

  it("gives none while a posted block is unreported, then the workers' once every one is", async () => {
    const h = await harness();
    await holdMoonInTwoBlocks(h);
    answerBlock(workerAt(h, 0), 0, blockWithLevels(0));
    const oneOfTwo = h.session.levels(MOON);
    answerBlock(workerAt(h, 0), 1, blockWithLevels(1_000));
    expect([oneOfTwo, h.session.levels(MOON)]).toEqual([null, blockWithLevels(1_000).levels]);
  });

  it("gives a lane of its one pool whose bake requests reach its workers", async () => {
    const h = await harness();
    await holdMoon(h);
    for (const worker of h.workers) {
      answerBlock(worker, 0, blockChange(5, [0, 2]));
    }
    const lane = h.session.lane(LANE);
    const key = { face: 2, level: 5, i: 0, j: 0 } as const;
    lane?.request({ key, priority: 1, forced: false });
    const bakes = h.workers.flatMap((w) => w.pendingBakes);
    lane?.terminate();
    expect([h.workers.length, bakes.map((b) => [b.key, b.settings])]).toEqual([2, [[key, LANE]]]);
  });

  it("gives neither once disposed", async () => {
    const h = await harness();
    await holdMoon(h);
    answerBlock(workerAt(h, 0), 0, blockChange(5, [0, 2]));
    h.session.dispose();
    expect([h.session.levels(MOON), h.session.lane(LANE)]).toEqual([null, null]);
  });
});

/** The scene's moon, which the contacts here are judged against. */
function sceneMoon(scene: ViewScene): ViewBody {
  const moon = scene.bodies.find((body) => body.id === MOON);
  if (moon === undefined) {
    throw new Error("the scene holds its moon");
  }
  return moon;
}

/** The session's moon turning (its rotation the identity) and carrying its own detail seed. */
const TURNING_MOON: ViewBody = {
  ...sceneMoon(SESSION_SCENE),
  rotation: IDENTITY_ROTATION,
  detailSeed: SEED,
};

/** The session's scene with {@link TURNING_MOON} for its moon. */
const SCENE: ViewScene = {
  ...SESSION_SCENE,
  bodies: SESSION_SCENE.bodies.map((body) => (body.id === MOON ? TURNING_MOON : body)),
};

/** The field's level, `moonLikeHeader`'s. */
const FIELD_LEVEL = 5;

/** The plateau's height above the datum that the session's bakes hold, m. */
const PLATEAU_M = 1_500;

/** A sorted list of cells as runs of one cell each. */
function runsOf(cells: ReadonlyArray<number>): number[] {
  return cells.toSorted((a, b) => a - b).flatMap((cell) => [cell, cell + 1]);
}

/**
 * A level table whose every level reaches `reachM` either side of the datum, its bounds ε_n 0 and
 * its spacings 1 m: room for a plateau's patch boxes.
 */
function levelsReaching(reachM: number): Float64Array {
  const table = new Float64Array(25 * 4);
  for (let n = 0; n < 25; n += 1) {
    table.set([0, -reachM, reachM, 1], 4 * n);
  }
  return table;
}

describe("the scene's craft as a pass's contacts on the workers' ground (R10.T14.c; decision-r10-t16-contracts §2)", () => {
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

  /** The pass's resident heights, their drawn ground a plateau 1.5 km above the datum. */
  const plateau = heldGroundUnder(
    { equatorialRadiusM: figure.equatorial_radius_m, polarRadiusM: figure.polar_radius_m },
    { groundHeightAtM: () => 1_500 },
  );

  it("takes a craft descending to a plateau more than 1 km above the datum as one contact on it", () => {
    // 800 m over the plateau, closing at 5 m/s: 2,300 m and 460 s from the datum.
    const contacts = sceneContacts(withCraft(2_300, -5), moon, plateau, 30);
    expect(contacts.map((c) => c.positionM.z)).toEqual([
      expect.closeTo(figure.polar_radius_m + 1_500, 3),
    ]);
  });

  it("takes none of that craft under the datum's rule", () => {
    expect(sceneContacts(withCraft(2_300, -5), moon, ground, 30)).toEqual([]);
  });

  /** The four field cells about the moon's north pole, the middle of face +z, in rising index. */
  const POLE_CELLS = [
    { i: 15, j: 15 },
    { i: 16, j: 15 },
    { i: 15, j: 16 },
    { i: 16, j: 16 },
  ]
    .map(({ i, j }) => fieldCellIndex({ face: 2, level: FIELD_LEVEL, i, j }))
    .toSorted((a, b) => a - b);

  /**
   * Answers every bake the workers hold as the plateau, {@link PLATEAU_M} above the datum at every
   * vertex and wholly surveyed, a bake sized as its settings ask; returns how many it answered.
   */
  function answerPlateau(h: Harness): number {
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
            originM: vec3(0, 0, figure.polar_radius_m),
            heights: new Float32Array(65 * 65 * 2).fill(PLATEAU_M),
            offsets: settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
            normals: new Float16Array(n * n * 2),
            heightRangeM: [PLATEAU_M, PLATEAU_M],
            boundingRadiusM: 1,
            originHeightM: PLATEAU_M,
            skirtDepthM: 1,
            layers: { ...layersOf("every"), fieldLevel: FIELD_LEVEL },
          },
        });
        answered += 1;
      }
    }
    return answered;
  }

  /** The finest patches the session's pass of the moon last selected as forced. */
  function forcedPatches(h: Harness): number {
    const selected = h.session.pass(MOON)?.lastStep?.selection.patches.values() ?? [];
    return [...selected].filter((patch) => patch.forced).length;
  }

  it("steps the session's pass with that craft as a contact on its resident bakes' ground", async () => {
    // The pass itself must judge the craft on the held ground: under the datum's rule it stays
    // 2,300 m up and 460 s out, no contact, and forces nothing once the plateau is resident.
    const h = await harness();
    h.session.noteView(PRIMARY, {
      ...viewAt(5e6, "photorealistic", { scene: withCraft(2_300, -5) }),
      regimes: new Map([[MOON, "terrain"]]),
    });
    h.session.step();
    const { id } = latestFieldAsk(h.socket());
    const bulk = sendBlocks(h);
    const cover = fieldCover(...POLE_CELLS.map((cell) => [cell, cell + 1, 30] as const));
    h.socket().serverResponds(id, readyField(figure, 1, cover, bulk));
    await settle();
    // The block's level table, posted before the pass is made: room for the plateau's boxes.
    for (const worker of h.workers) {
      answerBlock(worker, 0, {
        ...blockChange(FIELD_LEVEL, runsOf(POLE_CELLS)),
        levels: levelsReaching(5_000),
      });
    }
    h.session.step();
    await settle();
    h.session.step();
    const before = forcedPatches(h);
    const answered = answerPlateau(h);
    h.session.step();
    expect([
      before,
      answered > 0,
      h.session.pass(MOON)?.heights.groundHeightAtM(vec3(0, 0, 1)),
      forcedPatches(h) > 0,
    ]).toEqual([0, true, PLATEAU_M, true]);
  });
});

/** Every survey-mask vertex of a bake surveyed, or none. */
type Surveyed = "every" | "none";

/** A body's layers whose survey mask is `surveyed` at every vertex, on a field of level 6. */
function layersOf(surveyed: Surveyed): TerrainLayers {
  return {
    palette: new Uint8Array(16).fill(PALETTE_NONE),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(65 * 65).fill(surveyed === "every" ? 255 : 0),
    fieldLevel: 6,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/**
 * A bake of `key` whose own heights stand `heightM` above the datum at every vertex, its morph
 * target 0, its survey mask `surveyed` throughout, as `residentHeights.test.ts` builds its bakes.
 */
function bakeOf(key: PatchKey, heightM: number, surveyed: Surveyed): BakedPatch {
  const heights = new Float32Array(65 * 65 * 2);
  for (let n = 0; n < 65 * 65; n += 1) {
    heights[2 * n] = heightM;
  }
  return {
    key,
    generation: 1,
    originM: vec3(0, 0, 0),
    heights,
    offsets: null,
    normals: new Float16Array(65 * 65 * 2),
    heightRangeM: [heightM, heightM],
    boundingRadiusM: 1,
    originHeightM: 0,
    skirtDepthM: 1,
    layers: layersOf(surveyed),
  };
}

/** WGS 84 under the test planet's level table, as `grounded.test.ts` takes it: finest level 19. */
const WGS84_PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const WGS84_PATCH_M = finestPatchSizeM(WGS84_PLANET);
/** A finest patch, `grounded.test.ts`' site. */
const FINEST: PatchKey = { face: 2, level: 19, i: 200_011, j: 300_007 };
/** The level of {@link FINEST}'s coarse ancestor. */
const COARSE_LEVEL = 9;
/** {@link FINEST}'s ancestor at {@link COARSE_LEVEL}. */
const COARSE: PatchKey = {
  face: FINEST.face,
  level: COARSE_LEVEL,
  i: Math.floor(FINEST.i / 2 ** (FINEST.level - COARSE_LEVEL)),
  j: Math.floor(FINEST.j / 2 ** (FINEST.level - COARSE_LEVEL)),
};
/** TEST HULL's bounding radius, m: a corner of its 8 m × 5 m stern 10 m aft, √(4² + 2.5² + 10²). */
const HULL_RADIUS_M = Math.hypot(4, 2.5, 10);

/** The point `heightM` above the datum over vertex (x, y) of `key`, body-fixed. */
function over(key: PatchKey, heightM: number, x = 32, y = 32): Vec3 {
  return vec3(...surfacePoint(WGS84_FIGURE, vertexDir(key, x, y), heightM));
}

describe("the contact on the resident bakes as the finest lands (R10.T14.c; decision-r10-t14c-leans §2)", () => {
  /** The ancestor's drawn ground, m above the datum. */
  const COARSE_GROUND_M = -100;
  /** The finest bakes' drawn ground, m above the datum: within the ancestor's ε of its ground. */
  const FINEST_GROUND_M = 40;
  /** A craft 500 m over the finest ground under {@link FINEST}'s middle, closing at 5 m/s. */
  const CRAFT_M = over(FINEST, FINEST_GROUND_M + 500);
  const CLOSING = scale(datumGroundUnder(WGS84_FIGURE)(CRAFT_M).normal, -5);
  /** {@link FINEST} and its eight neighbours, every finest patch within r_g + a patch of the craft. */
  const BLOCK: PatchKey[] = [-1, 0, 1].flatMap((di) =>
    [-1, 0, 1].map((dj): PatchKey => ({
      face: FINEST.face,
      level: FINEST.level,
      i: FINEST.i + di,
      j: FINEST.j + dj,
    })),
  );

  /** The pass's resident heights with the patch cache's baked ranges beside them. */
  class Pass {
    readonly heights = new ResidentHeights();
    readonly #ranges = new Map<string, readonly [number, number]>();
    readonly ranges: HeightRangeLookup = {
      heightRangeM: (key) => this.#ranges.get(patchKeyString(key)),
    };

    /** A bake landing: resident, and in the cache. */
    land(bake: BakedPatch): void {
      this.heights.take(bake);
      this.#ranges.set(patchKeyString(bake.key), bake.heightRangeM);
    }

    /** The craft's contact on the drawn ground the pass holds. */
    contact(): GroundContact | null {
      return craftContact(
        CRAFT_M,
        CLOSING,
        HULL_RADIUS_M,
        heldGroundUnder(WGS84_FIGURE, this.heights),
        WGS84_PATCH_M,
      );
    }

    /** Selection about `contact`, over the cache's ranges. */
    select(contact: GroundContact): Selection {
      return selectPatches({
        planet: WGS84_PLANET,
        views: [viewOfCraft()],
        setting: "high",
        grounded: [contact],
        heightRanges: this.ranges,
      });
    }
  }

  /** A camera 300 m above and 400 m beside the craft, looking at it, at the high setting's τ. */
  function viewOfCraft(): ViewSelectionInput {
    const up = normalise(CRAFT_M);
    const east = normalise(vec3(-up.y, up.x, 0));
    const camera = add(add(CRAFT_M, scale(up, 300)), scale(east, 400));
    const orientation: Quaternion = lookAlong(sub(CRAFT_M, camera), up);
    return {
      camera: { positionM: camera, orientation },
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 1920, heightPx: 1080 },
      weight: 1,
      tauPx: TERRAIN_SETTINGS.high.tauPx,
    };
  }

  /** The point `heightM` above the datum along the normal through the craft's foot. */
  function groundUnderCraft(heightM: number): Vec3 {
    const under = datumGroundUnder(WGS84_FIGURE)(CRAFT_M);
    return add(under.groundPointM, scale(under.normal, heightM));
  }

  /** Whether each patch of {@link BLOCK} is selected as forced. */
  function forcedBlock(selection: Selection): boolean[] {
    return BLOCK.map((key) => selection.patches.get(patchKeyString(key))?.forced === true);
  }

  /** The pass with the ancestor alone resident, then with the finest block landed. */
  function landed(): { readonly before: GroundContact | null; readonly pass: Pass } {
    const pass = new Pass();
    pass.land(bakeOf(COARSE, COARSE_GROUND_M, "every"));
    const before = pass.contact();
    for (const key of BLOCK) {
      pass.land(bakeOf(key, FINEST_GROUND_M, "every"));
    }
    return { before, pass };
  }

  it("holds the two grounds within the ancestor's ε of each other, and over its forced radius apart", () => {
    // R09's bound: the finest ground lies within ε of the ancestor's level of its ground, so the
    // fixture is one a body could bake; and apart by more than the forced region reaches.
    const apartM = FINEST_GROUND_M - COARSE_GROUND_M;
    expect([
      apartM <= levelBoundM(WGS84_PLANET, COARSE_LEVEL),
      apartM > HULL_RADIUS_M + 3 * WGS84_PATCH_M,
    ]).toEqual([true, true]);
  });

  it("stands the contact on the ancestor's read ground while it alone is resident", () => {
    const { before } = landed();
    expect(
      before === null ? Infinity : norm(sub(before.positionM, groundUnderCraft(COARSE_GROUND_M))),
    ).toBeLessThan(1e-6);
  });

  it("moves the contact to the finest bake's read ground once it lands", () => {
    const { pass } = landed();
    const after = pass.contact();
    expect(
      after === null ? Infinity : norm(sub(after.positionM, groundUnderCraft(FINEST_GROUND_M))),
    ).toBeLessThan(1e-6);
  });

  it("forces the finest level under the contact on the ancestor's ground, whose boxes it widens by ε", () => {
    const pass = new Pass();
    pass.land(bakeOf(COARSE, COARSE_GROUND_M, "every"));
    const contact = pass.contact();
    expect(contact === null ? [] : forcedBlock(pass.select(contact))).toEqual(
      BLOCK.map(() => true),
    );
  });

  it("holds the finest level under the contact once it has moved to the finest ground", () => {
    const { pass } = landed();
    const after = pass.contact();
    expect(after === null ? [] : forcedBlock(pass.select(after))).toEqual(BLOCK.map(() => true));
  });

  it("would let the finest level go under a contact left on the ancestor's ground", () => {
    // The finest bakes' boxes stand at their own ground, 140 m above the ancestor's: beyond the
    // forced radius of a contact that had not converged.
    const { before, pass } = landed();
    expect(before === null ? [] : forcedBlock(pass.select(before))).toEqual(BLOCK.map(() => false));
  });
});

/** The parametric unit direction of `p` on `figure`, which the held ground reads its heights at. */
function parametricOf(figure: BodyFigure, p: Vec3): Vec3 {
  return normalise(
    vec3(p.x / figure.equatorialRadiusM, p.y / figure.equatorialRadiusM, p.z / figure.polarRadiusM),
  );
}

describe("the held ground as the datum's bit for bit where no surveyed bake is resident (R10.T14.c; decision-r10-t14c-leans §2)", () => {
  /** A sphere of WGS 84's equatorial radius. */
  const SPHERE: BodyFigure = {
    equatorialRadiusM: WGS84_FIGURE.equatorialRadiusM,
    polarRadiusM: WGS84_FIGURE.equatorialRadiusM,
    pole: null,
  };
  const FIGURES: ReadonlyArray<BodyFigure> = [WGS84_FIGURE, SPHERE];

  /** Points over {@link FINEST} on `figure`, inside {@link COARSE}: above and below the datum. */
  function pointsOn(figure: BodyFigure): Vec3[] {
    return [
      { heightM: 500, x: 32, y: 32 },
      { heightM: -50, x: 24, y: 40 },
      { heightM: 2_000, x: 40, y: 28 },
    ].map(({ heightM, x, y }) => vec3(...surfacePoint(figure, vertexDir(FINEST, x, y), heightM)));
  }

  it("is the datum's off the survey, over a resident bake whose mask surveys nothing", () => {
    const heights = new ResidentHeights();
    heights.take(bakeOf(COARSE, 800, "none"));
    for (const figure of FIGURES) {
      const points = pointsOn(figure);
      // The bare bake is found, and reads the datum, 0, rather than none.
      expect(points.map((p) => heights.groundHeightAtM(parametricOf(figure, p)))).toEqual([
        0, 0, 0,
      ]);
      expect(points.map(heldGroundUnder(figure, heights))).toStrictEqual(
        points.map(datumGroundUnder(figure)),
      );
    }
  });

  it("is the datum's where no bake is resident, and again once the only one is forgotten", () => {
    const forgotten = new ResidentHeights();
    forgotten.take(bakeOf(COARSE, 800, "every"));
    forgotten.forget(patchKeyString(COARSE));
    for (const heights of [new ResidentHeights(), forgotten]) {
      for (const figure of FIGURES) {
        const points = pointsOn(figure);
        expect(points.map(heldGroundUnder(figure, heights))).toStrictEqual(
          points.map(datumGroundUnder(figure)),
        );
      }
    }
  });
});

/**
 * The highest elevation of any cell the terrain golden's payloads carry, m above the datum: each
 * block's synthesis records' `elevation_mm` (`crates/hyperion-surface/src/wire.rs`' layout,
 * format 1: a 33-byte header, block 0's field header behind its `u16` length, the cover's ranges
 * of 9 bytes behind their `u32` count, then 26 bytes a cell, `elevation_mm` an `i32` first).
 *
 * @throws Error for a block of another format, or a block whose cells do not add up to its
 *   payload's coverage, either of which this reader would misread.
 */
function highestHeldCellM(golden: TerrainGolden): number {
  let highestMm = Number.NEGATIVE_INFINITY;
  for (const payload of golden.payloads) {
    let cells = 0;
    for (const hex of payload.blocksHex) {
      const bytes = blockBytes(hex);
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      if (view.getUint16(4, true) !== 1) {
        throw new Error("terrain_test.golden holds a block of a format this reader does not know");
      }
      let at = 33;
      if (view.getUint32(25, true) === 0) {
        at += 2 + view.getUint16(at, true);
      }
      const ranges = view.getUint32(at, true);
      at += 4;
      let blockCells = 0;
      for (let k = 0; k < ranges; k += 1) {
        blockCells += view.getUint32(at + 4, true) - view.getUint32(at, true);
        at += 9;
      }
      for (let n = 0; n < blockCells; n += 1) {
        highestMm = Math.max(highestMm, view.getInt32(at + 26 * n, true));
      }
      cells += blockCells;
    }
    const covered = payload.coverage.ranges.reduce((sum, { start, end }) => sum + end - start, 0);
    if (cells !== covered) {
      throw new Error(
        `a payload's blocks hold ${String(cells)} cells, its cover ${String(covered)}`,
      );
    }
  }
  return highestMm / 1_000;
}

describe("TERRAIN TEST on the workers' ground (R10.T14.c; decision-r10-t14c-leans §2)", () => {
  it("is never a contact at any whole second, over ground as high as the fixture's highest cell", () => {
    // The stand-in's ground, at every direction: the highest elevation of any cell the fixture
    // serves, surveyed or margin, plus ε at the fixture's finest level, level 17, which is 0 m
    // there (the workers' level table over the whole field at revision 2 is 0 from level 17 on,
    // measured 2026-10-20), so nothing is added. Why it stands above the ground the session's
    // contacts read: off the survey that ground is the datum, below the stand-in; on it, a resident
    // bake's own heights, R09's spline over the cells held, which never leaves their elevations
    // (`worker.rs`' `level_table`), with the levels' contributions on it. Those the workers' table
    // bounds only loosely (decision-r10-t14c-leans F4), so this is the ruling's conservative
    // stand-in rather than a proof: the surveyed ground under the T16.c marks lies 1.5 to 0.15 km
    // below the datum, kilometres under the highest cell, and a higher ground only makes a contact
    // likelier. The ship's lowest point is 4 km over the datum (`TERRAIN_LOWEST_ALTITUDE_M`).
    const golden = terrainTestGolden();
    const { header } = golden;
    const figure = {
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
      pole: null,
    };
    const highestM = highestHeldCellM(golden);
    const ground = heldGroundUnder(figure, { groundHeightAtM: () => highestM });
    const planet = planetGeometry(figure, null);
    const patchM = finestPatchSizeM(planet);
    const scene = terrainScene();
    const contacts = Array.from(
      { length: TERRAIN_TEST_SCRIPT.durationS + 1 },
      (_, tS) => tS,
    ).flatMap((tS) => {
      const at = scene.sceneAt(tS);
      const moon = at.bodies.find((body) => body.id === golden.body);
      return moon === undefined ? [null] : sceneContacts(at, moon, ground, patchM);
    });
    expect([planet.finestLevel, highestM > 0, contacts]).toEqual([17, true, []]);
  });
});

/** The speed of light, m/s (SI, exact). */
const C_M_PER_S = 299_792_458;

/**
 * The Sun's GM, m³/s²: IAU 2015 Resolution B3's nominal (GM)ᴺ☉ (Prša et al. 2016, AJ 152, 41).
 */
const GM_SUN_M3_S2 = 1.327_124_4e20;

/** A body's retarded state at `centreM` whose velocity is `presentMPerS` less a τ. */
function retardedOf(
  centreM: Vec3,
  presentMPerS: Vec3,
  accelerationMPerS2: Vec3,
  lightTimeS: number,
): RetardedCentre {
  return {
    centreM,
    velocityMPerS: sub(presentMPerS, scale(accelerationMPerS2, lightTimeS)),
    lightTimeS,
  };
}

/** A body's retarded velocity, m/s; NaN where its retarded state is not known. */
function retardedVelocityOf(body: ViewBody): Vec3 {
  return body.retarded?.velocityMPerS ?? vec3(Number.NaN, Number.NaN, Number.NaN);
}

/** The scene's one craft. */
function craftOf(scene: ViewScene): ViewCraft {
  const [craft] = scene.craft;
  if (craft === undefined) {
    throw new Error("the scene holds its craft");
  }
  return craft;
}

describe("a craft held in another body's frame (R10.T14.c; decision-r10-t16-leans §13, Q3; decision-r10-t14c-leans §5)", () => {
  // A Mars-like planet P and a Phobos-like moon M (decision-r10-t14c-leans §6):
  // - GM_P 42,828.37362 km³ s⁻² (JPL SSD, Planetary Satellite Physical Parameters, Mars, ephemeris
  //   MAR097), taken to seven figures;
  // - M's GM 7.087 × 10⁻⁴ km³ s⁻² (JPL SSD, Planetary Satellite Physical Parameters, ephemeris
  //   MAR097);
  // - its orbit's a 9,375 km and e 0.015 (JPL SSD, Planetary Satellite Mean Elements, MAR099; older
  //   tables give 9,376 km);
  // - its mean radius 11.08 km, of semi-axes 13.0 × 11.4 × 9.1 km (IAU WGCCRE 2015: Archinal et
  //   al. 2018, Celest. Mech. Dyn. Astron. 130:22).
  // Its Hill radius is then 16.56 km at a and 16.32 km at pericentre, a(1 − e), 1.495 of its
  // radius at a; the scene's `hillRadiusM`, as the server's is, holds the pericentre's. R02 holds
  // the own ship in M's frame from 0.9 of it, 14.7 km from M's centre (14.9 km at a's), 3.6 km
  // above its mean radius: a craft these tests hold in P's frame nearer M than that, within 1 km
  // of its ground, models one its source holds there, a served craft or a kept scene's, not the
  // own ship, which is still held in P's frame only in the band beyond. M moves at the circular
  // speed at a, 2.137 km/s, and the light times are 0 unless P is drawn as the local body.
  const GM_P_M3_S2 = 4.282_837e13;
  const GM_M_M3_S2 = 7.087e5;
  const RADIUS_M = 11_080;
  const ORBIT_M = 9.375e6;
  const ECCENTRICITY = 0.015;
  /** The Hill radius a (m ÷ 3M)^⅓ (Murray and Dermott 1999, §3.5) of a moon M at `aM`, m. */
  const hillM = (aM: number): number => aM * Math.cbrt(GM_M_M3_S2 / (3 * GM_P_M3_S2));
  /**
   * M's Hill radius at the pericentre of an orbit of semi-major axis `aM`, m, as the scene holds it:
   * a(1 − e)(m ÷ 3M)^⅓ (Hamilton and Burns 1992, Icarus 96, 43), as the server's `hill_radius`
   * (`crates/hyperion-sim/src/planetary/derive/limits.rs`).
   */
  const pericentreHillM = (aM: number): number => hillM(aM * (1 - ECCENTRICITY));
  /**
   * P 1.5237 au out, Mars's semi-major axis (JPL SSD's Keplerian elements), moving at 24.08 km/s,
   * Mars's mean orbital speed (NASA NSSDCA, Mars Fact Sheet; √(GM☉ ÷ a)(1 − e² ÷ 4) gives
   * 24.076 km/s), along +y, turning (its rotation the identity).
   */
  const P_CENTRE_M = vec3(1.5237 * AU_M, 0, 0);
  const P_VELOCITY_M_PER_S = vec3(0, 24_080, 0);
  /** M's velocity against P at `aM`, the circular speed √(GM_P ÷ a) along −x, m/s. */
  const orbitalMPerS = (aM: number): Vec3 => vec3(-Math.sqrt(GM_P_M3_S2 / aM), 0, 0);
  /** M's trailing side, +x, where a craft co-moving with M read against P alone would close. */
  const TRAILING = vec3(1, 0, 0);
  /** The craft at 0.95 of M's Hill radius, in R02's band between its 0.9 entry and its exit. */
  const BAND_M = 0.95 * pericentreHillM(ORBIT_M);

  /** How P is drawn: as every body but the local one, at its emission, or at the present. */
  interface PlanetDrawn {
    /** Whether P's retarded state is known. */
    readonly known?: boolean;
    /**
     * P as the scene's local body, drawn at the present: its present velocity, and its retarded
     * one that less its a·τ ({@link LOCAL}), with M's retarded one likewise its present one less
     * its own a·τ. Absent, a kept scene's light time 0, where the retarded velocity is the present
     * one.
     */
    readonly local?: boolean;
  }

  /**
   * The light times and accelerations of a served scene whose observer, the own ship, stands in
   * M's band ({@link BAND_M} from its centre, still held in P's frame): P's light time 9,375 km ÷ c
   * and its acceleration GM☉ ÷ r² towards the star, m/s²; M's light time {@link BAND_M} ÷ c and its
   * acceleration GM_P ÷ a² towards P.
   */
  const LOCAL = {
    planetLightTimeS: ORBIT_M / C_M_PER_S,
    planetAccelerationMPerS2: scale(normalise(P_CENTRE_M), -GM_SUN_M3_S2 / norm(P_CENTRE_M) ** 2),
    moonLightTimeS: BAND_M / C_M_PER_S,
    moonAccelerationMPerS2: vec3(0, -GM_P_M3_S2 / ORBIT_M ** 2, 0),
  } as const;

  /**
   * The scene with P and M, M at `aM` from P along +y, with one craft at `fromMM` from M's centre
   * in the frame `kind` (P's `body` or `body_fixed` frame, or the system's), at `velocityMPerS` in
   * that frame.
   */
  function marsScene(
    aM: number,
    kind: "body" | "body_fixed" | "system",
    fromMM: Vec3,
    velocityMPerS: Vec3,
    drawn: PlanetDrawn = {},
  ): { readonly scene: ViewScene; readonly p: ViewBody; readonly m: ViewBody } {
    const mCentre = add(P_CENTRE_M, vec3(0, aM, 0));
    const mPresentMPerS = add(P_VELOCITY_M_PER_S, orbitalMPerS(aM));
    const local = drawn.local === true;
    const p = aBody({
      designation: "MARS-LIKE PLANET",
      radiusM: 3.3895e6,
      centreM: P_CENTRE_M,
      retarded:
        drawn.known === false
          ? null
          : retardedOf(
              P_CENTRE_M,
              P_VELOCITY_M_PER_S,
              LOCAL.planetAccelerationMPerS2,
              local ? LOCAL.planetLightTimeS : 0,
            ),
      ...(local ? { presentVelocityMPerS: P_VELOCITY_M_PER_S } : {}),
      rotation: IDENTITY_ROTATION,
    });
    const m = aBody({
      id: FIXTURE_MOON,
      parent: FIXTURE_PLANET,
      kind: "moon",
      designation: "PHOBOS-LIKE MOON",
      radiusM: RADIUS_M,
      hillRadiusM: pericentreHillM(aM),
      centreM: mCentre,
      retarded: retardedOf(
        mCentre,
        mPresentMPerS,
        LOCAL.moonAccelerationMPerS2,
        local ? LOCAL.moonLightTimeS : 0,
      ),
      rotation: IDENTITY_ROTATION,
    });
    let position: ViewPosition;
    switch (kind) {
      case "body":
      case "body_fixed":
        position = { kind, body: FIXTURE_PLANET, m: add(vec3(0, aM, 0), fromMM) };
        break;
      case "system":
        position = { kind, system: FIXTURE_SYSTEM, m: add(mCentre, fromMM) };
        break;
    }
    const craft = aViewCraft({ pose: { position, attitude: NO_TURN }, velocityMPerS });
    return {
      scene: aViewScene({
        ...(local ? {} : { provenance: { kind: "kept" as const, name: "PHOBOS TEST" } }),
        bodies: [p, m],
        craft: [craft],
      }),
      p,
      m,
    };
  }

  /** M's ground under a body-fixed point: the datum of its mean sphere. */
  const mGround = datumGroundUnder({ equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M });

  it("gives the Phobos-like moon a Hill radius of 16.56 km at a and 16.32 km at pericentre, 1.495 of its radius, and 2.14 km/s", () => {
    expect([
      hillM(ORBIT_M) / 1e3,
      pericentreHillM(ORBIT_M) / 1e3,
      hillM(ORBIT_M) / RADIUS_M,
      norm(orbitalMPerS(ORBIT_M)) / 1e3,
    ]).toEqual([
      expect.closeTo(16.56, 2),
      expect.closeTo(16.32, 2),
      expect.closeTo(1.495, 3),
      expect.closeTo(2.14, 2),
    ]);
  });

  it("takes a craft held in the planet's frame in the moon's band, closing on the moon, as one contact on it", () => {
    // 4.42 km above M, closing at 200 m/s against it: 22 s from contact.
    const { scene, m } = marsScene(
      ORBIT_M,
      "body",
      scale(TRAILING, BAND_M),
      add(orbitalMPerS(ORBIT_M), scale(TRAILING, -200)),
    );
    expect(sceneContacts(scene, m, mGround, 30).map((c) => c.positionM.x)).toEqual([
      expect.closeTo(RADIUS_M, 3),
    ]);
  });

  it("takes none of the craft at rest against the moon, which closes on it at 2.14 km/s against the planet", () => {
    const { scene, m } = marsScene(ORBIT_M, "body", scale(TRAILING, BAND_M), orbitalMPerS(ORBIT_M));
    expect(sceneContacts(scene, m, mGround, 30)).toEqual([]);
  });

  it("chains nothing where the planet's retarded state is not known", () => {
    const { scene, m } = marsScene(
      ORBIT_M,
      "body",
      scale(TRAILING, BAND_M),
      add(orbitalMPerS(ORBIT_M), scale(TRAILING, -200)),
      { known: false },
    );
    expect(sceneContacts(scene, m, mGround, 30)).toEqual([]);
  });

  it("takes a craft 500 m over a moon whose Hill radius is 1.05 of its radius, closing at 2 m/s, as a contact", () => {
    // A hypothetical moon: M's mass and radius, 6,585 km from P. Its Hill radius sets where it
    // stands against its own rigid Roche limit d_R, whatever the densities: R_H ÷ R =
    // (a ÷ d_R)(2 ÷ 3)^⅓, so 1.05 puts it at 1.20 d_R, d_R ≈ 5,478 km. R02's 0.9 entry then lies
    // below M's surface, so a craft is held in P's frame all the way down. On M's leading side,
    // −x, closing against M is receding at 2.55 km/s against P alone.
    const aM = (1.05 * RADIUS_M) / Math.cbrt(GM_M_M3_S2 / (3 * GM_P_M3_S2));
    const leading = vec3(-1, 0, 0);
    const { scene, m } = marsScene(
      aM,
      "body",
      scale(leading, RADIUS_M + 500),
      add(orbitalMPerS(aM), scale(leading, -2)),
    );
    expect([
      0.9 * (m.hillRadiusM ?? Infinity) < RADIUS_M,
      sceneContacts(scene, m, mGround, 30).length,
    ]).toEqual([true, 1]);
  });

  it("takes a craft held in the planet's fixed frame as a contact only while grounded", () => {
    // Its velocity is against P's turning frame, whose rate the scene does not hold.
    const at = (heightM: number): ReturnType<typeof marsScene> =>
      marsScene(
        ORBIT_M,
        "body_fixed",
        scale(TRAILING, RADIUS_M + heightM),
        add(orbitalMPerS(ORBIT_M), scale(TRAILING, -2)),
      );
    const grounded = at(20);
    const descending = at(500);
    expect([
      sceneContacts(grounded.scene, grounded.m, mGround, 30).length,
      sceneContacts(descending.scene, descending.m, mGround, 30).length,
    ]).toEqual([1, 0]);
  });

  it("reads a craft co-moving with the moon at rest against it, to rounding, and closing at 2.14 km/s without the chain", () => {
    const { scene, p, m } = marsScene(
      ORBIT_M,
      "body",
      scale(TRAILING, BAND_M),
      orbitalMPerS(ORBIT_M),
    );
    const craft = craftOf(scene);
    const againstMoonMPerS = craftVelocityAgainst(craft, m, scene);
    // Against P alone, its velocity in its own frame, along M's normal on its trailing side.
    const againstPlanetMPerS = craftVelocityAgainst(craft, p, scene);
    expect([
      againstMoonMPerS === null ? Infinity : norm(againstMoonMPerS),
      againstPlanetMPerS === null ? null : dot(againstPlanetMPerS, TRAILING) / 1e3,
    ]).toEqual([expect.closeTo(0, 9), expect.closeTo(-2.14, 2)]);
  });

  describe("with the planet the local body, drawn at the present", () => {
    it("chains through the planet's present velocity, not its retarded one, which differs by its a·τ", () => {
      const vMPerS = add(orbitalMPerS(ORBIT_M), scale(TRAILING, -200));
      const { scene, p, m } = marsScene(ORBIT_M, "body", scale(TRAILING, BAND_M), vMPerS, {
        local: true,
      });
      const pRetardedMPerS = retardedVelocityOf(p);
      const mRetardedMPerS = retardedVelocityOf(m);
      const chainedMPerS = craftVelocityAgainst(craftOf(scene), m, scene);
      const aTauMPerS = norm(LOCAL.planetAccelerationMPerS2) * LOCAL.planetLightTimeS;
      expect([
        chainedMPerS,
        norm(sub(P_VELOCITY_M_PER_S, pRetardedMPerS)),
        chainedMPerS === null
          ? null
          : norm(sub(chainedMPerS, add(vMPerS, sub(pRetardedMPerS, mRetardedMPerS)))),
      ]).toEqual([
        add(vMPerS, sub(P_VELOCITY_M_PER_S, mRetardedMPerS)),
        expect.closeTo(aTauMPerS, 12),
        expect.closeTo(aTauMPerS, 9),
      ]);
    });

    it("reads an own-ship stand-in co-moving with the moon as its barycentric velocity less the moon's retarded one", () => {
      // The stand-in held in P's frame at v_obs − V_P^present (`viewSceneFromServer`), v_obs the
      // moon's present velocity: it reads V_obs − V_M^ret, the moon's a·τ alone, |a_M| τ_M, where
      // P's retarded velocity would have added P's a·τ too.
      const vObsMPerS = add(P_VELOCITY_M_PER_S, orbitalMPerS(ORBIT_M));
      const { scene, m } = marsScene(
        ORBIT_M,
        "body",
        scale(TRAILING, BAND_M),
        sub(vObsMPerS, P_VELOCITY_M_PER_S),
        { local: true },
      );
      const mRetardedMPerS = retardedVelocityOf(m);
      const readMPerS = craftVelocityAgainst(craftOf(scene), m, scene);
      expect([
        readMPerS === null ? Infinity : norm(sub(readMPerS, sub(vObsMPerS, mRetardedMPerS))),
        readMPerS === null ? null : norm(readMPerS),
      ]).toEqual([
        expect.closeTo(0, 9),
        expect.closeTo(norm(LOCAL.moonAccelerationMPerS2) * LOCAL.moonLightTimeS, 10),
      ]);
    });

    it("subtracts the local body's present velocity in the system's branch, and the moon's retarded one", () => {
      const vMPerS = vec3(1_000, 25_000, -3);
      const { scene, p, m } = marsScene(ORBIT_M, "system", scale(TRAILING, BAND_M), vMPerS, {
        local: true,
      });
      const craft = craftOf(scene);
      expect([
        craftVelocityAgainst(craft, p, scene),
        craftVelocityAgainst(craft, m, scene),
      ]).toEqual([sub(vMPerS, P_VELOCITY_M_PER_S), sub(vMPerS, retardedVelocityOf(m))]);
    });

    it("gives a craft held in the planet's fixed frame no velocity against the moon", () => {
      const { scene, m } = marsScene(
        ORBIT_M,
        "body_fixed",
        scale(TRAILING, BAND_M),
        orbitalMPerS(ORBIT_M),
        { local: true },
      );
      expect(craftVelocityAgainst(craftOf(scene), m, scene)).toBeNull();
    });
  });

  it("gives a kept scene, no body at the present and light time 0, the retarded velocities' chain as before", () => {
    const vMPerS = add(orbitalMPerS(ORBIT_M), scale(TRAILING, -200));
    const held = marsScene(ORBIT_M, "body", scale(TRAILING, BAND_M), vMPerS);
    const system = marsScene(ORBIT_M, "system", scale(TRAILING, BAND_M), vMPerS);
    expect([
      held.scene.provenance.kind,
      held.scene.bodies.map((body) => body.presentVelocityMPerS),
      held.scene.bodies.map((body) => body.retarded?.lightTimeS),
      craftVelocityAgainst(craftOf(held.scene), held.m, held.scene),
      craftVelocityAgainst(craftOf(system.scene), system.m, system.scene),
    ]).toEqual([
      "kept",
      [undefined, undefined],
      [0, 0],
      add(vMPerS, sub(retardedVelocityOf(held.p), retardedVelocityOf(held.m))),
      sub(vMPerS, retardedVelocityOf(system.m)),
    ]);
  });
});
