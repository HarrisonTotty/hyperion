/**
 * A body's terrain pass shared by the views that hand it over (plan R10, R10.T16.b): one
 * selection a step over every view, each view's own draw, the level table, R10's rebake rule and
 * the gate it holds the body's mask behind, the survey, the contacts, the fault and the releases,
 * over a scripted pool and R01's counting engine.
 */

import { describe, expect, it, vi } from "vitest";

import { instrumentViewId, PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { add, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lawFor } from "../appearance/phase";
import { lookAlong, rotate } from "../camera/quaternion";
import type { ViewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import type { MemoryCategory } from "../engine/memory";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";
import { TERRAIN_NOT_AVAILABLE } from "./annunciation";
import { uvToSt, vertexDir, xyzToFaceUv } from "./cube";
import { allocationTally } from "./gpu/allocationTally";
import { normalsAtlasOf } from "./gpu/resources";
import { INSTANCE_RECORD_BYTES, NO_PARENT_SLOT } from "./gpu/uniforms";
import { craftContact, datumGroundUnder, finestPatchSizeM } from "./grounded";
import { MAX_LEVEL, patchKeyString, type PatchKey } from "./patchKey";
import { type PlanetGeometry, planetGeometry } from "./planet";
import { PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "./priority";
import { type PatchRequest, type Selection, type SelectionInput, selectPatches } from "./select";
import { RESELECT_MOVE_FRACTION } from "./selectionTolerance";
import { terrainDepthMaterialSpec, type TerrainLighting } from "./terrainMaterial";
import {
  DEFAULT_TERRAIN_VIEW,
  frustumMarginRad,
  MAX_FRUSTUM_MARGIN_RAD,
  morphRangeM,
  TerrainPass,
  type TerrainPassOptions,
  type TerrainPassView,
  type TerrainPool,
  type TerrainSurveyLookup,
  type TerrainView,
  type TerrainViewFrame,
  type TerrainViewFrameInput,
} from "./terrainPass";
import {
  type BakedPatch,
  type BakeSettings,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

const PLANET: PlanetGeometry = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** The primary's and an instrument's IDs, as `VIEW` gives them. */
const PRIMARY = PRIMARY_VIEW_ID;
const INSTRUMENT = instrumentViewId(1);

/** A pool that records each lane's demand and bakes on the test's command. */
class ScriptedLane implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  /** Each demand given, in order. */
  readonly demands: Array<ReadonlyArray<PatchRequest>> = [];
  /** R10's layers each bake carries unless a key has its own. */
  layers: TerrainLayers | null = null;
  readonly layersOf = new Map<string, TerrainLayers>();
  unavailable = false;
  terminated = false;
  readonly #baked = new Set<(bake: BakedPatch) => void>();
  readonly #failed = new Set<(failure: { readonly keyString: string }) => void>();

  constructor(readonly settings: BakeSettings) {}

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
    this.demands.push(demand);
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#baked.add(cb);
    return () => {
      this.#baked.delete(cb);
    };
  }

  onFailed(cb: (failure: { readonly keyString: string }) => void): () => void {
    this.#failed.add(cb);
    return () => {
      this.#failed.delete(cb);
    };
  }

  terminate(): void {
    this.terminated = true;
  }

  /** Answers every demanded patch with a flat bake; returns how many. */
  bakeDemand(pass: TerrainPass): number {
    const demand = this.demand;
    for (const request of demand) {
      this.deliver(pass, request.key);
    }
    return demand.length;
  }

  deliver(pass: TerrainPass, key: PatchKey): void {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: { x: 1_000, y: 2_000, z: PLANET.figure.polarRadiusM },
      heights: new Float32Array(65 * 65 * 2),
      offsets: this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
      normals: new Float16Array(n * n * 2),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
      originHeightM: 0,
      skirtDepthM: 1,
      layers: this.layersOf.get(patchKeyString(key)) ?? this.layers,
    };
    for (const listener of this.#baked) {
      listener(bake);
    }
  }

  fail(key: PatchKey): void {
    for (const listener of this.#failed) {
      listener({ keyString: patchKeyString(key) });
    }
  }
}

/** A camera `heightM` above the north pole, looking straight down. */
function northPole(heightM: number): TerrainView {
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + heightM },
    orientation: lookAlong({ x: 0, y: 0, z: -1 }, { x: 1, y: 0, z: 0 }),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 640, heightPx: 360 },
  };
}

/** A camera 1,000 km above the equator over `dir`, looking straight down. */
function overEquator(dir: Vec3, widthPx = 640): TerrainView {
  const r = PLANET.figure.equatorialRadiusM + 1_000_000;
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: scale(dir, r),
    orientation: lookAlong(scale(dir, -1), { x: 0, y: 0, z: 1 }),
    fovXRad: Math.PI / 3,
    viewport: { widthPx, heightPx: (widthPx * 9) / 16 },
  };
}

/** A view of the pass at the high setting's τ. */
function passView(id: ViewId, view: TerrainView, weight = PRIMARY_VIEW_WEIGHT): TerrainPassView {
  return { id, view, tauPx: TERRAIN_SETTINGS.high.tauPx, weight };
}

/** A view's frame inputs, at `nowMs`. */
function frameOf(view: TerrainView, nowMs = 0, lighting?: TerrainLighting): TerrainViewFrameInput {
  return {
    view,
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    ...(lighting === undefined ? {} : { lighting }),
    nowMs,
  };
}

/** A body's lit source, its bakes with their weights. */
const BODY = {
  kind: "body",
  body: "0200080020000000.0301",
  detailSeed: "617373656d626c79",
  weights: "baked",
} as const;

/** A lit frame's lights: none, under the Moon's template. */
const LIGHTING: TerrainLighting = {
  lights: [],
  occluders: [],
  secondaries: [],
  elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
  exposureScale: 1e-4,
};

/** A body's layers, complete or not, with its vertices outside the survey in `unsurveyed`. */
function layers(complete = true, unsurveyed: ReadonlyArray<number> = []): TerrainLayers {
  const palette = new Uint8Array(16).fill(PALETTE_NONE);
  palette.set([1, 0, 2, PALETTE_NONE]);
  const weights = new Uint8Array(65 * 65 * 8);
  const photometry = new Float32Array(40);
  photometry.set([0.1, 0.12, 0.14, 1, 1]);
  return {
    palette,
    weights,
    surveyMask: new Uint8Array(65 * 65).fill(unsurveyed.length === 0 ? 255 : 0),
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete,
    unsurveyedCells: Uint32Array.from(unsurveyed),
    photometry,
    horizonNear: new Uint32Array(65 * 65 * 4),
    horizonFar: new Uint32Array(9 * 9 * 4),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/** A counting engine with the device limits the passes' caches take. */
function engineOf(): Promise<CountingRenderEngine> {
  return countingRenderEngine({
    maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
    maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  });
}

interface Made {
  readonly pass: TerrainPass;
  readonly engine: CountingRenderEngine;
  /** The pass's current lane. */
  readonly lane: () => ScriptedLane;
  readonly lanes: ScriptedLane[];
}

/** A shared pass on `setting` over `engine`, ready. */
async function sharedPass(
  setting: "high" | "low",
  extra: Partial<
    Pick<TerrainPassOptions, "source" | "survey" | "onSelect" | "planet" | "frustumMargin">
  > = {},
  given?: CountingRenderEngine,
): Promise<Made> {
  const engine = given ?? (await engineOf());
  const lanes: ScriptedLane[] = [];
  const { source = { kind: "test-planet", ridges: "off" }, ...rest } = extra;
  const pass = new TerrainPass({
    engine,
    setting,
    planet: PLANET,
    source,
    createPool: (bake) => {
      const lane = new ScriptedLane(bake);
      lanes.push(lane);
      return lane;
    },
    ...rest,
  });
  await pass.ready();
  return {
    pass,
    engine,
    lanes,
    lane: () => {
      const lane = lanes.at(-1);
      if (lane === undefined) {
        throw new Error("no lane was taken");
      }
      return lane;
    },
  };
}

/** The slots and morph bands of the instance records last written to `view`'s buffer. */
function records(
  engine: CountingRenderEngine,
  view: string,
): Array<{ readonly slot: number; readonly morph: readonly [number, number] }> {
  const write = engine.writes.findLast((w) => w.buffer === `terrain instances (${view})`);
  const bytes = write?.data.slice().buffer ?? new ArrayBuffer(0);
  const u = new Uint32Array(bytes);
  const f = new Float32Array(bytes);
  const out: Array<{ readonly slot: number; readonly morph: readonly [number, number] }> = [];
  for (let n = 0; n < u.length / (INSTANCE_RECORD_BYTES / 4); n += 1) {
    out.push({ slot: u[n * 8 + 3] ?? -1, morph: [f[n * 8 + 4] ?? 0, f[n * 8 + 5] ?? 0] });
  }
  return out;
}

/** Steps `pass` over `views` and bakes its demand, `rounds` times. */
function stream(made: Made, views: ReadonlyArray<TerrainPassView>, rounds: number): void {
  for (let n = 0; n < rounds; n += 1) {
    made.pass.step({ views, grounded: [] });
    made.lane().bakeDemand(made.pass);
  }
  made.pass.step({ views, grounded: [] });
}

describe("one selection over every view that hands the body over (R10.T16.b)", () => {
  it("asks for each patch once for two views at one pose, as for one", async () => {
    const two = await sharedPass("high");
    const one = await sharedPass("high");
    const view = northPole(2_000_000);
    two.pass.step({ views: [passView(PRIMARY, view), passView(INSTRUMENT, view)], grounded: [] });
    one.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    const keys = two.lane().demand.map((r) => patchKeyString(r.key));
    expect(new Set(keys).size).toBe(keys.length);
    expect(two.lane().demand).toEqual(one.lane().demand);
  });

  it("ranks a patch only an instrument wants at a quarter of the primary's", async () => {
    const primary = northPole(2_000_000);
    const instrument = overEquator(vec3(1, 0, 0));
    const quarter = await sharedPass("high");
    const whole = await sharedPass("high");
    const alone = await sharedPass("high");
    quarter.pass.step({
      views: [passView(PRIMARY, primary), passView(INSTRUMENT, instrument, SECONDARY_VIEW_WEIGHT)],
      grounded: [],
    });
    whole.pass.step({
      views: [passView(PRIMARY, primary), passView(INSTRUMENT, instrument, PRIMARY_VIEW_WEIGHT)],
      grounded: [],
    });
    alone.pass.step({ views: [passView(PRIMARY, primary)], grounded: [] });
    const primaryKeys = new Set(alone.lane().demand.map((r) => patchKeyString(r.key)));
    const priority = new Map(whole.lane().demand.map((r) => [patchKeyString(r.key), r.priority]));
    const instrumentOnly = quarter
      .lane()
      .demand.filter((r) => !primaryKeys.has(patchKeyString(r.key)));
    expect(instrumentOnly.length).toBeGreaterThan(0);
    for (const request of instrumentOnly) {
      expect(request.priority / (priority.get(patchKeyString(request.key)) ?? 0)).toBeCloseTo(
        SECONDARY_VIEW_WEIGHT,
        12,
      );
    }
  });

  it("selects as the pure selection of its inputs, two views and a survey", async () => {
    const inputs: SelectionInput[] = [];
    const survey: TerrainSurveyLookup = { surveyedUnder: (key) => key.face !== 3, version: 1 };
    const made = await sharedPass("high", { survey, onSelect: (input) => inputs.push(input) });
    const views = [
      passView(PRIMARY, northPole(2_000_000)),
      passView(INSTRUMENT, overEquator(vec3(1, 0, 0)), SECONDARY_VIEW_WEIGHT),
    ];
    stream(made, views, 3);
    const input = inputs.at(-1);
    if (input === undefined) {
      throw new Error("the pass did not select");
    }
    expect(input.survey).toBe(survey);
    const fresh = selectPatches(input);
    expect([...(made.pass.lastStep?.selection.patches.keys() ?? [])]).toEqual([
      ...fresh.patches.keys(),
    ]);
  });
});

describe("each view's own draw of the shared pass (R10.T16.b)", () => {
  it("draws for each view only the drawn patches it sees, at its own morph bands", async () => {
    const made = await sharedPass("high");
    const primary = northPole(2_000_000);
    // The instrument over the equator, its picture twice as wide: its bands twice as far.
    const instrument = overEquator(vec3(1, 0, 0), 1280);
    const views = [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)];
    stream(made, views, 4);
    made.pass.drawFor(PRIMARY, frameOf(primary));
    made.pass.drawFor(INSTRUMENT, frameOf(instrument));
    const step = made.pass.lastStep;
    if (step === null) {
      throw new Error("the pass did not select");
    }
    const { drawSet, selection } = step;
    const slotsOf = (bit: number): number[] =>
      drawSet.patches
        .filter((_, n) => ((drawSet.views[n] ?? 0) & (1 << bit)) !== 0)
        .map((p) => p.patch.slot);
    const level = new Map(drawSet.patches.map((p) => [p.patch.slot, p.patch.key.level]));
    const primaryRecords = records(made.engine, PRIMARY);
    const instrumentRecords = records(made.engine, INSTRUMENT);
    expect(primaryRecords.map((r) => r.slot)).toEqual(slotsOf(0));
    expect(instrumentRecords.map((r) => r.slot)).toEqual(slotsOf(1));
    expect(slotsOf(0)).not.toEqual(slotsOf(1));
    expect(selection.limited).toBe(false);
    for (const [view, written] of [
      [primary, primaryRecords],
      [instrument, instrumentRecords],
    ] as const) {
      for (const { slot, morph } of written) {
        const bands = morphRangeM(PLANET, level.get(slot) ?? 0, {
          camera: { positionM: view.cameraM, orientation: view.orientation },
          fovXRad: view.fovXRad,
          viewport: view.viewport,
          weight: PRIMARY_VIEW_WEIGHT,
          tauPx: TERRAIN_SETTINGS.high.tauPx,
        });
        expect(morph).toEqual(bands.map((m) => Math.fround(m)));
      }
    }
  });

  it("releases a leaving view's own buffers alone, and the other draws on with its own", async () => {
    const made = await sharedPass("high");
    const primary = northPole(2_000_000);
    const instrument = overEquator(vec3(1, 0, 0));
    stream(made, [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)], 3);
    made.pass.drawFor(PRIMARY, frameOf(primary));
    made.pass.drawFor(INSTRUMENT, frameOf(instrument));
    const destroyed: string[] = [];
    made.engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    made.pass.step({ views: [passView(PRIMARY, primary)], grounded: [] });
    const frame = made.pass.drawFor(PRIMARY, frameOf(primary, 16));
    expect(destroyed.toSorted()).toEqual(
      [
        `terrain contacts (${INSTRUMENT})`,
        `terrain indirect (${INSTRUMENT})`,
        `terrain instances (${INSTRUMENT})`,
      ].toSorted(),
    );
    expect(frame.draw?.storageBuffers?.["instances"]?.name).toBe(`terrain instances (${PRIMARY})`);
    expect(made.pass.drawFor(INSTRUMENT, frameOf(instrument)).draw).toBeNull();
  });

  it("gives a view the last step did not take no draw", async () => {
    const made = await sharedPass("high");
    const view = northPole(2_000_000);
    stream(made, [passView(PRIMARY, view)], 2);
    const frame = made.pass.drawFor(INSTRUMENT, frameOf(view));
    expect([frame.draw, frame.fault, frame.maskReady]).toEqual([null, null, false]);
  });
});

describe("the shared pass's level table (R10.T16.b)", () => {
  it("selects again on a widened table, with the table's own bounds, and not on the same one", async () => {
    const inputs: SelectionInput[] = [];
    const made = await sharedPass("high", { onSelect: (input) => inputs.push(input) });
    const views = [passView(PRIMARY, northPole(2_000_000))];
    expect(made.pass.step({ views, grounded: [] }).reselected).toBe(true);
    expect(made.pass.step({ views, grounded: [] }).reselected).toBe(false);
    made.pass.setLevels(goldenLevelTable("off"));
    expect(made.pass.step({ views, grounded: [] }).reselected).toBe(false);
    const widened = goldenLevelTable("off").map((value, n) => (n % 4 === 3 ? value : 2 * value));
    made.pass.setLevels(widened);
    expect(made.pass.step({ views, grounded: [] }).reselected).toBe(true);
    expect(inputs.at(-1)?.planet.levels).toEqual(widened);
    expect(made.pass.planet.levels).toEqual(widened);
  });
});

describe("the shared pass's rebakes (R10.T16.b; R10 Design note 16)", () => {
  /** A pass over the north pole with face 2's root and its four children resident. */
  async function streamed(): Promise<{ readonly made: Made; readonly views: TerrainPassView[] }> {
    const made = await sharedPass("low");
    const views = [passView(PRIMARY, northPole(2_000_000))];
    made.lane().layers = layers();
    stream(made, views, 2);
    return { made, views };
  }

  it("asks again, at its last priority, for exactly the resident patches the rule names", async () => {
    const { made, views } = await streamed();
    const resident = [...(made.pass.lastStep?.drawSet.patches ?? [])].map((p) => p.patch.key);
    const [named, passedOver, incomplete] = resident;
    if (named === undefined || passedOver === undefined || incomplete === undefined) {
      throw new Error("too few patches are drawn");
    }
    // Rebake each of the three with layers the block's change does or does not name.
    made.lane().layersOf.set(patchKeyString(named), layers(true, [5]));
    made.lane().layersOf.set(patchKeyString(passedOver), layers(true, [9]));
    made.lane().layersOf.set(patchKeyString(incomplete), layers(false));
    for (const key of [named, passedOver, incomplete]) {
      made.lane().deliver(made.pass, key);
    }
    made.pass.step({ views, grounded: [] });
    const lastAsk = new Map(
      made
        .lane()
        .demands.flat()
        .map((r) => [patchKeyString(r.key), r] as const),
    );
    let settled = 0;
    made.pass.fieldChanged(
      { newlySurveyed: Uint32Array.of(5, 7), newlyHeld: Uint32Array.of(5, 7) },
      () => {
        settled += 1;
      },
    );
    made.pass.step({ views, grounded: [] });
    const asked = made.lane().demand.map((r) => patchKeyString(r.key));
    expect(asked.filter((k) => k === patchKeyString(passedOver))).toEqual([]);
    const rebakes = made
      .lane()
      .demand.filter((r) =>
        [named, incomplete].some((k) => patchKeyString(k) === patchKeyString(r.key)),
      );
    expect(
      rebakes.map((r) => [patchKeyString(r.key), r.priority, r.forced]).toSorted(byKey),
    ).toEqual(
      [named, incomplete]
        .map((k) => {
          const last = lastAsk.get(patchKeyString(k));
          return [patchKeyString(k), last?.priority, last?.forced];
        })
        .toSorted(byKey),
    );
    // Drawn until their new bakes replace them, in their own slots.
    const slot = made.pass.resident(patchKeyString(named))?.slot;
    expect(settled).toBe(0);
    made.lane().layersOf.set(patchKeyString(named), layers());
    made.lane().deliver(made.pass, named);
    expect([made.pass.resident(patchKeyString(named))?.slot, settled]).toEqual([slot, 0]);
    made.lane().layersOf.set(patchKeyString(incomplete), layers());
    made.lane().deliver(made.pass, incomplete);
    expect([settled, made.pass.settled]).toEqual([1, true]);
    made.pass.step({ views, grounded: [] });
    const again = new Set([named, incomplete].map(patchKeyString));
    expect(made.lane().demand.filter((r) => again.has(patchKeyString(r.key)))).toEqual([]);
  });

  it("tells a block at once where the rule names nothing", async () => {
    const { made } = await streamed();
    let settled = 0;
    made.pass.fieldChanged(
      { newlySurveyed: Uint32Array.of(1, 2), newlyHeld: new Uint32Array() },
      () => {
        settled += 1;
      },
    );
    expect(settled).toBe(1);
  });

  /**
   * A streamed pass whose first drawn patch's bake is incomplete, so that a block names it for a
   * rebake; `told` counts the gates settled.
   */
  async function waitingOnRebake(): Promise<{
    readonly made: Made;
    readonly key: PatchKey;
    readonly told: { count: number };
    readonly tell: () => void;
  }> {
    const { made } = await streamed();
    const key = made.pass.lastStep?.drawSet.patches[0]?.patch.key;
    if (key === undefined) {
      throw new Error("nothing is drawn");
    }
    made.lane().layersOf.set(patchKeyString(key), layers(false));
    made.lane().deliver(made.pass, key);
    const told = { count: 0 };
    return {
      made,
      key,
      told,
      tell: () => {
        told.count += 1;
      },
    };
  }

  it("holds a block that names a resident bake until its rebake", async () => {
    const { made, told, tell } = await waitingOnRebake();
    made.pass.fieldChanged(
      { newlySurveyed: Uint32Array.of(1, 2), newlyHeld: new Uint32Array() },
      tell,
    );
    expect(told.count).toBe(0);
  });

  it("tells a block once its rebakes are given up", async () => {
    const { made, key, told, tell } = await waitingOnRebake();
    made.pass.fieldChanged(
      { newlySurveyed: Uint32Array.of(1, 2), newlyHeld: new Uint32Array() },
      tell,
    );
    made.lane().fail(key);
    expect(told.count).toBe(1);
  });

  it("tells a block waiting on its rebakes when it is rebuilt", async () => {
    const { made, told, tell } = await waitingOnRebake();
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      tell,
    );
    made.engine.restore();
    expect(told.count).toBe(1);
  });

  it("tells a block given after it is disposed at once", async () => {
    const { made, told, tell } = await waitingOnRebake();
    made.pass.dispose();
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      tell,
    );
    expect(told.count).toBe(1);
  });

  it("tells a block waiting on its rebakes when it is disposed", async () => {
    const { made, told, tell } = await waitingOnRebake();
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      tell,
    );
    made.pass.dispose();
    expect(told.count).toBe(1);
  });
});

describe("the shared pass over a survey (R10.T16.b; decision-r10-t16-contracts §2)", () => {
  it("asks for, draws and streams nothing over unsurveyed ground", async () => {
    const survey: TerrainSurveyLookup = { surveyedUnder: (key) => key.face === 5, version: 1 };
    const made = await sharedPass("high", { survey });
    const view = northPole(2_000_000);
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(view, 0));
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, 2_000));
    expect(made.lane().demand).toEqual([]);
    expect([frame.draw, frame.conditions.streaming, frame.annunciation, frame.maskReady]).toEqual([
      null,
      false,
      null,
      true,
    ]);
  });

  it("says a view sees a drawn patch only over the survey, once one is drawn (R10.T16.d)", async () => {
    const away: TerrainSurveyLookup = { surveyedUnder: (key) => key.face === 5, version: 1 };
    const off = await sharedPass("low", { source: BODY, survey: away });
    const view = northPole(2_000_000);
    off.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    const made = await sharedPass("low", { source: BODY });
    made.lane().layers = layers();
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    const before = made.pass.seesPatch(PRIMARY);
    stream(made, [passView(PRIMARY, view)], 6);
    expect([
      off.pass.seesPatch(PRIMARY),
      before,
      made.pass.seesPatch(PRIMARY),
      made.pass.seesPatch(INSTRUMENT),
    ]).toEqual([false, false, true, false]);
  });

  it("streams while a surveyed patch waits for its bake", async () => {
    const survey: TerrainSurveyLookup = { surveyedUnder: (key) => key.face === 2, version: 1 };
    const made = await sharedPass("high", { survey });
    const view = northPole(2_000_000);
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(view, 0));
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, 300));
    expect(made.lane().demand.length).toBeGreaterThan(0);
    expect(made.lane().demand.every((r) => r.key.face === 2)).toBe(true);
    expect([frame.missing, frame.maskReady, frame.annunciation]).toEqual([
      true,
      false,
      "TERRAIN: STREAMING",
    ]);
  });

  it("selects again while it closes on a survey out of view, its patches entering within a step", async () => {
    // A survey about the north pole; the camera 1,000 km up over the south pole, looking down and
    // never turning, flies up a meridian until the north face rises over its horizon.
    const cells = new Set(["2/8/127/127", "2/8/128/127", "2/8/127/128", "2/8/128/128"]);
    const survey: TerrainSurveyLookup = {
      surveyedUnder: (key) => {
        for (const cell of cells) {
          const [face, level, i, j] = cell.split("/").map(Number);
          if (face !== key.face || level === undefined || i === undefined || j === undefined) {
            continue;
          }
          const shift = key.level - level;
          const same =
            shift >= 0
              ? key.i >> shift === i && key.j >> shift === j
              : i >> -shift === key.i && j >> -shift === key.j;
          if (same) {
            return true;
          }
        }
        return false;
      },
      version: 1,
    };
    const inputs: SelectionInput[] = [];
    const made = await sharedPass("high", { survey, onSelect: (input) => inputs.push(input) });
    const r = PLANET.figure.equatorialRadiusM + 1_000_000;
    const orientation = lookAlong({ x: 0, y: 0, z: 1 }, { x: 1, y: 0, z: 0 });
    const poses: TerrainView[] = Array.from({ length: 601 }, (_, n) => {
      const angle = -Math.PI / 2 + (n / 600) * ((3 * Math.PI) / 4);
      return {
        rotation: IDENTITY_ROTATION,
        cameraM: vec3(r * Math.cos(angle), 0, r * Math.sin(angle)),
        orientation,
        fovXRad: Math.PI / 3,
        viewport: { widthPx: 640, heightPx: 360 },
      };
    });
    let firstFresh = -1;
    let firstHeld = -1;
    const reselectedAt: number[] = [];
    poses.forEach((view, n) => {
      const step = made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
      if (step.reselected) {
        reselectedAt.push(n);
      }
      const input = inputs.at(-1);
      if (input === undefined) {
        throw new Error("the pass did not select");
      }
      const fresh = selectPatches({
        ...input,
        views: [
          { ...(input.views[0] ?? fail()), camera: { positionM: view.cameraM, orientation } },
        ],
      });
      if (firstFresh < 0 && fresh.patches.size > 0) {
        firstFresh = n;
      }
      if (firstHeld < 0 && step.selection.patches.size > 0) {
        firstHeld = n;
      }
    });
    expect(firstFresh).toBeGreaterThan(0);
    expect(firstHeld).toBeGreaterThanOrEqual(firstFresh);
    // It selected again on the way, while nothing of the survey was in view.
    expect(reselectedAt.filter((n) => n > 0 && n < firstFresh).length).toBeGreaterThan(1);
    // The survey's patches entered at the first selection at or after the first pose that holds
    // them, within one step of the move rule from the selection before it.
    const before = reselectedAt.findLast((n) => n < firstFresh) ?? 0;
    expect(reselectedAt).toContain(firstHeld);
    expect(reselectedAt.find((n) => n >= firstFresh)).toBe(firstHeld);
    const at = (n: number): Vec3 => poses[n]?.cameraM ?? vec3(0, 0, 0);
    const moved = (a: number, b: number): number => {
      const p = at(a);
      const q = at(b);
      return Math.hypot(p.x - q.x, p.y - q.y, p.z - q.z);
    };
    const nearest = selectPatches({
      ...(inputs[reselectedAt.indexOf(before)] ?? fail()),
    }).nearestSurveyM;
    expect(moved(before, firstHeld)).toBeLessThanOrEqual(
      RESELECT_MOVE_FRACTION * nearest + moved(firstHeld - 1, firstHeld),
    );
  });
});

/** Orders rows by their first column, a key string. */
function byKey(a: readonly unknown[], b: readonly unknown[]): number {
  return String(a[0]).localeCompare(String(b[0]));
}

/** Fails the test where a value it needs is missing. */
function fail(): never {
  throw new Error("a value the test needs is missing");
}

describe("the shared pass's contacts (R10.T16.b; R05 Design note 9)", () => {
  const ground = datumGroundUnder(PLANET.figure);
  const up = normalise(vec3(0.1, 0.2, 1));
  const above = (heightM: number): Vec3 => {
    const foot = ground(scale(up, PLANET.figure.polarRadiusM)).groundPointM;
    return { x: foot.x + up.x * heightM, y: foot.y + up.y * heightM, z: foot.z + up.z * heightM };
  };

  const patchM = finestPatchSizeM(PLANET);

  /** The patches the pass forces in one step with `grounded`'s contacts, from 2,000 km up. */
  async function forced(grounded: ReturnType<typeof craftContact>[]): Promise<number> {
    const made = await sharedPass("high");
    const view = northPole(2_000_000);
    return [
      ...made.pass
        .step({ views: [passView(PRIMARY, view)], grounded: grounded.flatMap((c) => c ?? []) })
        .selection.patches.values(),
    ].filter((p) => p.forced).length;
  }

  it("forces the region under a craft at 900 m and closing", async () => {
    const closing = craftContact(above(900), scale(up, -5), 10.3, ground, patchM);
    expect(await forced([closing])).toBeGreaterThan(0);
  });

  it("takes no contact of a craft at 2 km and receding", () => {
    expect(craftContact(above(2_000), scale(up, 5), 10.3, ground, patchM)).toBeNull();
  });

  it("forces nothing with no contact", async () => {
    expect(await forced([])).toBe(0);
  });
});

describe("the shared pass's fault (R10.T16.b; decision-r10-t16-contracts §7)", () => {
  it("asks for nothing and draws nothing once every worker is lost, each view's frame carrying the fault", async () => {
    const made = await sharedPass("high");
    const view = northPole(2_000_000);
    stream(made, [passView(PRIMARY, view)], 2);
    let settled = 0;
    const key = made.pass.lastStep?.drawSet.patches[0]?.patch.key;
    if (key === undefined) {
      throw new Error("nothing is drawn");
    }
    made.lane().layersOf.set(patchKeyString(key), layers(false));
    made.lane().deliver(made.pass, key);
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      () => {
        settled += 1;
      },
    );
    made.lane().unavailable = true;
    const step = made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, 5_000));
    expect([made.lane().demand, step.drawSet.count, made.pass.faulted, settled]).toEqual([
      [],
      0,
      true,
      1,
    ]);
    expect([frame.draw, frame.annunciation, frame.fault, frame.maskReady]).toEqual([
      null,
      null,
      TERRAIN_NOT_AVAILABLE,
      false,
    ]);
    // Any view's frame carries it, and a later block is told at once.
    expect(made.pass.drawFor(INSTRUMENT, frameOf(view)).fault).toBe(TERRAIN_NOT_AVAILABLE);
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      () => {
        settled += 1;
      },
    );
    expect(settled).toBe(2);
  });
});

/** Live bytes by category, from a tally made before the pass. */
const CATEGORIES: ReadonlyArray<MemoryCategory> = ["height-cache", "other", "coarse-field-gpu"];

describe("the shared pass's releases (R10.T16.b; decision-r10-t16-contracts §1)", () => {
  it.each([
    ["a restore after a loss", true],
    ["a restore", false],
  ] as const)(
    "leaves no handle of its own once made, drawn, rebuilt by %s and disposed",
    async (_name, lose) => {
      const engine = await engineOf();
      const tally = allocationTally(engine);
      const before = engine.live;
      const bytes = CATEGORIES.map((c) => tally.liveBytes(c));
      const made = await sharedPass("low", { source: BODY }, engine);
      made.lane().layers = layers();
      const primary = northPole(2_000_000);
      const instrument = overEquator(vec3(1, 0, 0));
      const views = [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)];
      stream(made, views, 2);
      made.pass.drawFor(PRIMARY, frameOf(primary, 0, LIGHTING));
      made.pass.drawFor(INSTRUMENT, frameOf(instrument, 0, LIGHTING));
      if (lose) {
        engine.loseHandles();
      }
      engine.restore();
      // The rebuild compiles the material again, which the counting engine answers at once.
      await Promise.resolve();
      made.lane().layers = layers();
      stream(made, views, 2);
      made.pass.drawFor(PRIMARY, frameOf(primary, 16, LIGHTING));
      made.pass.dispose();
      expect(engine.live).toEqual(before);
      expect(CATEGORIES.map((c) => tally.liveBytes(c))).toEqual(bytes);
    },
  );
});

describe("the shared pass for a view's frame (R10.T16.c)", () => {
  it("compiles a lit pass's depth material for the symbology", async () => {
    const lit = await sharedPass("low", { source: BODY });
    expect(lit.pass.symbologyDepthMaterial?.name).toBe(
      terrainDepthMaterialSpec(lit.pass.layout.vertexPath).name,
    );
  });

  it("compiles no depth material for the spike's shading", async () => {
    const spike = await sharedPass("low");
    expect(spike.pass.symbologyDepthMaterial).toBeNull();
  });

  describe("a view's mask, ready only as its draw is", () => {
    const view = northPole(2_000_000);

    /** A lit pass selected once over the primary, then streamed `rounds` rounds. */
    async function madeFor(rounds: number): Promise<Made> {
      const made = await sharedPass("low", { source: BODY });
      made.lane().layers = layers();
      made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
      stream(made, [passView(PRIMARY, view)], rounds);
      return made;
    }

    // Before any bake lands every patch the view sees is missing.
    it("is withheld while every patch the view sees is missing", async () => {
      const made = await madeFor(0);
      expect([
        made.pass.maskReadyFor(PRIMARY),
        made.pass.drawFor(PRIMARY, frameOf(view, 0, LIGHTING)).maskReady,
      ]).toEqual([false, false]);
    });

    it("is given once nothing the view sees is missing and its draw is made", async () => {
      const made = await madeFor(6);
      expect([
        made.pass.maskReadyFor(PRIMARY),
        made.pass.drawFor(PRIMARY, frameOf(view, 16, LIGHTING)).maskReady,
      ]).toEqual([true, true]);
    });

    it("is not given to a view the pass does not draw", async () => {
      const made = await madeFor(6);
      expect(made.pass.maskReadyFor(INSTRUMENT)).toBe(false);
    });
  });

  it("keeps the heights of its resident bakes that hold the edge, and forgets them at a restore", async () => {
    const made = await sharedPass("low", { source: BODY });
    const edge = layers();
    // One vertex outside the survey: the bake's mask holds the edge.
    edge.surveyMask[0] = 0;
    made.lane().layers = edge;
    const view = northPole(2_000_000);
    stream(made, [passView(PRIMARY, view)], 2);
    const drawn = made.pass.lastStep?.drawSet.patches[0]?.patch.key;
    if (drawn === undefined) {
      throw new Error("the pass drew no patch");
    }
    const [x, y, z] = vertexDir(drawn, 32, 32);
    const at = vec3(x, y, z);
    const before = made.pass.heights.rimHeightAtM(at);
    made.engine.restore();
    expect([before, made.pass.heights.rimHeightAtM(at)]).toEqual([0, null]);
  });
});

/** The bytes of the records last written to the buffer named `name`. */
function recordBytes(engine: CountingRenderEngine, name: string): Uint8Array {
  return engine.writes.findLast((w) => w.buffer === name)?.data ?? new Uint8Array();
}

/** The parent slots of the instance records in `bytes`, their seventh words. */
function parentSlots(bytes: Uint8Array): number[] {
  const words = new Uint32Array(bytes.slice().buffer);
  const out: number[] = [];
  for (let n = 6; n < words.length; n += INSTANCE_RECORD_BYTES / 4) {
    out.push(words[n] ?? NO_PARENT_SLOT);
  }
  return out;
}

/** A body's whole layers on a field of `fieldLevel`, its vertices outside the survey `unsurveyed`. */
function layersAt(fieldLevel: number, unsurveyed: ReadonlyArray<number> = []): TerrainLayers {
  return { ...layers(true, unsurveyed), fieldLevel };
}

describe("the live path's records are the single view's (R10.T16; T8.b's sunset seams)", () => {
  /** A lit pass over the body, its bakes with horizon maps, so that its parents' slots are real. */
  async function litPass(): Promise<Made> {
    const made = await sharedPass("high", { source: BODY });
    made.lane().layers = layers();
    return made;
  }

  it("writes the primary's records as the descent spike's one view does, bit for bit, parents' slots included", async () => {
    const view = northPole(2_000_000);
    const single = await litPass();
    const live = await litPass();
    for (let n = 0; n < 5; n += 1) {
      single.pass.frame({ ...frameOf(view, n, LIGHTING), grounded: [] });
      single.lane().bakeDemand(single.pass);
      live.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
      live.lane().bakeDemand(live.pass);
    }
    single.pass.frame({ ...frameOf(view, 99, LIGHTING), grounded: [] });
    live.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    live.pass.drawFor(PRIMARY, frameOf(view, 99, LIGHTING));
    const spike = recordBytes(single.engine, "terrain instances");
    const primary = recordBytes(live.engine, `terrain instances (${PRIMARY})`);
    expect([
      [...primary].join(),
      parentSlots(primary).some((slot) => slot !== NO_PARENT_SLOT),
    ]).toEqual([[...spike].join(), true]);
  });

  it("writes an instrument that joins after the first step as the primary at its pose", async () => {
    const view = northPole(2_000_000);
    const made = await litPass();
    stream(made, [passView(PRIMARY, view)], 4);
    const both = [passView(PRIMARY, view), passView(INSTRUMENT, view, SECONDARY_VIEW_WEIGHT)];
    made.pass.step({ views: both, grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(view, 50, LIGHTING));
    made.pass.drawFor(INSTRUMENT, frameOf(view, 50, LIGHTING));
    expect([...recordBytes(made.engine, `terrain instances (${INSTRUMENT})`)].join()).toBe(
      [...recordBytes(made.engine, `terrain instances (${PRIMARY})`)].join(),
    );
  });

  it("writes an instrument that leaves and joins again as the primary at its pose, its bands made again", async () => {
    const view = northPole(2_000_000);
    const made = await litPass();
    const both = [passView(PRIMARY, view), passView(INSTRUMENT, view, SECONDARY_VIEW_WEIGHT)];
    stream(made, both, 4);
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    made.pass.step({ views: both, grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(view, 50, LIGHTING));
    made.pass.drawFor(INSTRUMENT, frameOf(view, 50, LIGHTING));
    expect([...recordBytes(made.engine, `terrain instances (${INSTRUMENT})`)].join()).toBe(
      [...recordBytes(made.engine, `terrain instances (${PRIMARY})`)].join(),
    );
  });
});

describe("the mask at survey cell code 0 (decision-r10-t16-review §2)", () => {
  /**
   * A lit pass over a field of level 6, whose roots alone are of cell code 0: the roots baked with
   * `rootUnsurveyed` vertices outside the survey, every other patch whole, streamed until the roots
   * have landed and the next step has selected again.
   */
  async function rootsLanded(
    rootUnsurveyed: ReadonlyArray<number>,
    planet: PlanetGeometry = PLANET,
  ): Promise<{ readonly made: Made; readonly view: TerrainView }> {
    const made = await sharedPass("high", { source: BODY, planet });
    made.lane().layers = layersAt(6);
    const view = northPole(2_000_000);
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    if (!made.lane().demand.every((r) => r.key.level === 0)) {
      throw new Error("the first demand is the roots'");
    }
    for (const request of made.lane().demand) {
      made.lane().layersOf.set(patchKeyString(request.key), layersAt(6, rootUnsurveyed));
    }
    made.lane().bakeDemand(made.pass);
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    return { made, view };
  }

  it("withholds the mask while a root whose bake holds a vertex outside the survey stands in, with only the stand-in's STREAMING", async () => {
    const { made, view } = await rootsLanded([0]);
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, 0, LIGHTING));
    expect([
      made.pass.maskReadyFor(PRIMARY),
      frame.maskReady,
      frame.standingIn,
      frame.missing,
      frame.conditions.streaming,
    ]).toEqual([false, false, true, false, true]);
  });

  it("gives the mask once the root's children have landed", async () => {
    const { made, view } = await rootsLanded([0]);
    made.lane().bakeDemand(made.pass);
    made.pass.step({ views: [passView(PRIMARY, view)], grounded: [] });
    expect([
      made.pass.maskReadyFor(PRIMARY),
      made.pass.drawFor(PRIMARY, frameOf(view, 16, LIGHTING)).maskReady,
    ]).toEqual([true, true]);
  });

  it("gives the mask while a root whose bake holds every vertex surveyed stands in", async () => {
    const { made, view } = await rootsLanded([]);
    expect([
      made.pass.maskReadyFor(PRIMARY),
      made.pass.drawFor(PRIMARY, frameOf(view, 0, LIGHTING)).maskReady,
    ]).toEqual([true, true]);
  });

  it("withholds the mask under a root kept as a steady leaf, and raises no STREAMING", async () => {
    // No relief in the level table: a root's error is its chord's sagitta, some hundreds of metres,
    // which subtends under τ from 2,000 km, so the roots are the selection's leaves.
    const flat = planetGeometry(WGS84_FIGURE, new Float64Array(goldenLevelTable("off").length));
    const { made, view } = await rootsLanded([0], flat);
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, 0, LIGHTING));
    expect([
      made.pass.maskReadyFor(PRIMARY),
      frame.maskReady,
      frame.standingIn,
      frame.missing,
      frame.conditions.streaming,
    ]).toEqual([false, false, false, false, false]);
  });
});

describe("the shared pass's rebakes given up (the batch review's S7)", () => {
  it("leaves a rebake the lane gave up out of every later block's gate", async () => {
    const made = await sharedPass("low");
    const views = [passView(PRIMARY, northPole(2_000_000))];
    made.lane().layers = layers();
    stream(made, views, 2);
    const key = made.pass.lastStep?.drawSet.patches[0]?.patch.key ?? fail();
    // An incomplete bake, which the rule names at every block.
    made.lane().layersOf.set(patchKeyString(key), layers(false));
    made.lane().deliver(made.pass, key);
    const block = { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() };
    made.pass.fieldChanged(block, () => undefined);
    made.lane().fail(key);
    let settled = 0;
    made.pass.fieldChanged(block, () => {
      settled += 1;
    });
    expect([settled, made.pass.settled]).toEqual([1, true]);
  });
});

describe("each view's own nearest survey (the batch review's S9)", () => {
  it("reselects a view over bare ground by its own distance to the survey, not another view's", async () => {
    // The survey is the north face; the primary 2,000 km over it, the instrument 1,000 km under the
    // south pole over bare ground, some 10,000 km from the nearest surveyed ground.
    const survey: TerrainSurveyLookup = { surveyedUnder: (key) => key.face === 2, version: 1 };
    const made = await sharedPass("high", { survey });
    const primary = northPole(2_000_000);
    const south = (offsetM: number): TerrainView => ({
      rotation: IDENTITY_ROTATION,
      cameraM: vec3(offsetM, 0, -(PLANET.figure.polarRadiusM + 1_000_000)),
      orientation: lookAlong({ x: 0, y: 0, z: 1 }, { x: 1, y: 0, z: 0 }),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 640, heightPx: 360 },
    });
    const views = (offsetM: number): TerrainPassView[] => [
      passView(PRIMARY, primary),
      passView(INSTRUMENT, south(offsetM), SECONDARY_VIEW_WEIGHT),
    ];
    stream(made, views(0), 2);
    // 300 km: past a tenth of the primary's 2,000 km, short of a tenth of the instrument's own.
    expect(made.pass.step({ views: views(300_000), grounded: [] }).reselected).toBe(false);
  });
});

/** The sphere of the Earth's mean radius, under the golden level table. */
const SPHERE: PlanetGeometry = planetGeometry(
  { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000, pole: null },
  goldenLevelTable("off"),
);

/**
 * How many of a 32 × 18 grid of `view`'s rays meet the datum of `planet` (a sphere) on no patch
 * that `selection`'s view 0 sees: a strip where its coverage mask would discard the reference
 * surface with no terrain selected.
 */
function unselectedRays(selection: Selection, view: TerrainView, planet: PlanetGeometry): number {
  const radiusM = planet.figure.equatorialRadiusM;
  const tanX = Math.tan(view.fovXRad / 2);
  const tanY = (tanX * view.viewport.heightPx) / view.viewport.widthPx;
  const p = view.cameraM;
  let unselected = 0;
  for (let gy = 0; gy < 18; gy += 1) {
    for (let gx = 0; gx < 32; gx += 1) {
      const x = (2 * (gx + 0.5)) / 32 - 1;
      const y = 1 - (2 * (gy + 0.5)) / 18;
      const d = normalise(rotate(view.orientation, vec3(x * tanX, y * tanY, -1)));
      const b = p.x * d.x + p.y * d.y + p.z * d.z;
      const disc = b * b - (p.x * p.x + p.y * p.y + p.z * p.z - radiusM * radiusM);
      if (disc < 0) {
        continue;
      }
      const t = -b - Math.sqrt(disc);
      const { face, u, v } = xyzToFaceUv([p.x + t * d.x, p.y + t * d.y, p.z + t * d.z]);
      const s = uvToSt(u);
      const r = uvToSt(v);
      let seen = false;
      for (let level = 0; level <= MAX_LEVEL && !seen; level += 1) {
        const n = 2 ** level;
        const key = {
          face,
          level,
          i: Math.min(n - 1, Math.floor(s * n)),
          j: Math.min(n - 1, Math.floor(r * n)),
        };
        seen = ((selection.patches.get(patchKeyString(key))?.views ?? 0) & 1) !== 0;
      }
      if (!seen) {
        unselected += 1;
      }
    }
  }
  return unselected;
}

describe("each view's selection margin for its motion (R10.T16; the batch review's S6)", () => {
  const FOV_X_RAD = Math.PI / 3;
  const VIEWPORT = { widthPx: 640, heightPx: 360 };
  const PIXEL_RAD = FOV_X_RAD / VIEWPORT.widthPx;
  const TURN_RAD = (2 * Math.PI) / 180;
  /**
   * A camera 5 km over the equator at 0° E, looking `pitchDeg` below the horizon, turned `yawRad`
   * east of north, over `fovXRad` across.
   */
  function low(yawRad: number, fovXRad = FOV_X_RAD, pitchDeg = 20): TerrainView {
    const up = vec3(1, 0, 0);
    const north = vec3(0, 0, 1);
    const east = vec3(0, 1, 0);
    const level = add(scale(north, Math.cos(yawRad)), scale(east, Math.sin(yawRad)));
    const pitch = (pitchDeg * Math.PI) / 180;
    const forward = add(scale(level, Math.cos(pitch)), scale(up, -Math.sin(pitch)));
    return {
      rotation: IDENTITY_ROTATION,
      cameraM: scale(up, SPHERE.figure.equatorialRadiusM + 5_000),
      orientation: lookAlong(forward, up),
      fovXRad,
      viewport: VIEWPORT,
    };
  }
  // An instrument's narrow field slewing 3° a frame, 6° below the horizon: the patches at its
  // leading edge, 50 to 100 km out, subtend less than its frame's turn. (Nearer, every patch's
  // bounds hold the camera, and the selection takes them whatever the frustum.)
  const NARROW_RAD = (10 * Math.PI) / 180;
  const SLEW_RAD = (3 * Math.PI) / 180;
  const slewing = (yawRad: number): TerrainView => low(yawRad, NARROW_RAD, 6);
  function selectFor(view: TerrainView, marginRad?: number): Selection {
    return selectPatches({
      planet: SPHERE,
      views: [
        {
          camera: { positionM: view.cameraM, orientation: view.orientation },
          fovXRad: view.fovXRad,
          viewport: view.viewport,
          weight: PRIMARY_VIEW_WEIGHT,
          tauPx: TERRAIN_SETTINGS.high.tauPx,
          ...(marginRad === undefined ? {} : { frustumMarginRad: marginRad }),
        },
      ],
      setting: "high",
      grounded: [],
    });
  }

  it("without one, leaves a strip of a slewing view's leading edge unselected a frame on", () => {
    expect(unselectedRays(selectFor(slewing(0)), slewing(SLEW_RAD), SPHERE)).toBeGreaterThan(0);
  });

  it("opened by a frame's turn and a pixel, selects every ray the view's next frame draws", () => {
    const pixelRad = NARROW_RAD / VIEWPORT.widthPx;
    expect(
      unselectedRays(selectFor(slewing(0), SLEW_RAD + pixelRad), slewing(SLEW_RAD), SPHERE),
    ).toBe(0);
  });

  it("selects as before, bit for bit, with a margin of 0", () => {
    const plain = selectFor(low(0));
    const zero = selectFor(low(0), 0);
    expect([[...zero.patches.keys()], zero.demand]).toEqual([
      [...plain.patches.keys()],
      plain.demand,
    ]);
  });

  it("is a pixel at a view's first step, then a pixel and the view's turn since its last, at most its cap", () => {
    const at = low(0);
    const view = {
      camera: { positionM: at.cameraM, orientation: at.orientation },
      fovXRad: FOV_X_RAD,
      viewport: VIEWPORT,
    };
    const turned = { positionM: at.cameraM, orientation: low(TURN_RAD).orientation };
    const away = { positionM: at.cameraM, orientation: low(Math.PI / 2).orientation };
    expect([
      frustumMarginRad(view, null, Number.POSITIVE_INFINITY),
      frustumMarginRad(view, turned, Number.POSITIVE_INFINITY),
      frustumMarginRad(view, away, Number.POSITIVE_INFINITY),
    ]).toEqual([PIXEL_RAD, expect.closeTo(PIXEL_RAD + TURN_RAD, 12), MAX_FRUSTUM_MARGIN_RAD]);
  });

  it("adds the angle its move turns its nearest ground by", () => {
    const at = low(0);
    const view = {
      camera: { positionM: at.cameraM, orientation: at.orientation },
      fovXRad: FOV_X_RAD,
      viewport: VIEWPORT,
    };
    const moved = { positionM: add(at.cameraM, vec3(0, 50, 0)), orientation: at.orientation };
    expect(frustumMarginRad(view, moved, 10_000)).toBeCloseTo(PIXEL_RAD + 50 / 10_000, 12);
  });

  it("opens a session's pass's views by their motion, and the spike's frame by none", async () => {
    const inputs: SelectionInput[] = [];
    const spike: SelectionInput[] = [];
    const moving = await sharedPass("high", {
      frustumMargin: "motion",
      onSelect: (input) => inputs.push(input),
    });
    const still = await sharedPass("high", { onSelect: (input) => spike.push(input) });
    moving.pass.step({ views: [passView(PRIMARY, low(0))], grounded: [] });
    moving.pass.step({ views: [passView(PRIMARY, low(TURN_RAD))], grounded: [] });
    still.pass.frame({ ...frameOf(low(0), 0), grounded: [] });
    expect([
      inputs.map((input) => input.views[0]?.frustumMarginRad),
      spike.map((input) => input.views[0] !== undefined && "frustumMarginRad" in input.views[0]),
    ]).toEqual([[PIXEL_RAD, expect.closeTo(PIXEL_RAD + TURN_RAD, 12)], [false]]);
  });
});

describe("the shared pass through a loss and its materials (the batch review's S11 and C9)", () => {
  it("draws nothing and throws nothing for a view whose buffers a loss refuses, its mask withheld", async () => {
    const made = await sharedPass("low", { source: BODY });
    made.lane().layers = layers();
    const view = northPole(2_000_000);
    stream(made, [passView(PRIMARY, view)], 2);
    made.engine.loseDevice();
    made.pass.step({ views: [passView(PRIMARY, view), passView(INSTRUMENT, view)], grounded: [] });
    const frame = made.pass.drawFor(INSTRUMENT, frameOf(view, 0, LIGHTING));
    expect([frame.draw, frame.maskReady]).toEqual([null, false]);
  });

  it("reports a depth material that fails to compile, and draws on without it", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await engineOf();
    engine.failMaterials((name) => name.startsWith("terrain-depth"));
    const made = await sharedPass("low", { source: BODY }, engine);
    made.lane().layers = layers();
    const view = northPole(2_000_000);
    stream(made, [passView(PRIMARY, view)], 2);
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, 0, LIGHTING));
    expect([
      made.pass.symbologyDepthMaterial,
      frame.draw !== null,
      error.mock.calls.length,
    ]).toEqual([null, true, 1]);
  });

  it("releases the materials compiled for a pass let go meanwhile", async () => {
    const engine = await engineOf();
    const pass = new TerrainPass({
      engine,
      setting: "low",
      planet: PLANET,
      source: BODY,
      createPool: (bake) => new ScriptedLane(bake),
    });
    const ready = pass.ready();
    pass.dispose();
    await ready;
    expect(engine.live.materials).toBe(0);
  });

  it("releases the materials compiled for a device a restore replaced, and keeps the new device's", async () => {
    const engine = await engineOf();
    const pass = new TerrainPass({
      engine,
      setting: "low",
      planet: PLANET,
      source: BODY,
      createPool: (bake) => new ScriptedLane(bake),
    });
    const ready = pass.ready();
    engine.restore();
    await ready;
    await Promise.resolve();
    expect([engine.live.materials, pass.symbologyDepthMaterial !== null]).toEqual([2, true]);
    pass.dispose();
  });

  it("says nothing of a material a loss refused to compile: the restore compiles it again", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await engineOf();
    const pass = new TerrainPass({
      engine,
      setting: "low",
      planet: PLANET,
      source: BODY,
      createPool: (bake) => new ScriptedLane(bake),
    });
    engine.loseDevice();
    await pass.ready();
    expect(error).not.toHaveBeenCalled();
  });

  it("leaves no byte of its mesh's either, its destroyed bytes those it made", async () => {
    const engine = await engineOf();
    const tally = allocationTally(engine);
    const before = tally.liveBytes("other");
    const made = await sharedPass("low", { source: BODY }, engine);
    const mesh = tally.liveBytes("other") - before;
    made.pass.dispose();
    expect([mesh > 0, tally.liveBytes("other")]).toEqual([true, before]);
  });
});

describe("the spike's default view (the batch review's C4)", () => {
  it("is no view's ID, and its buffers go by R05's names, made at its first frame", async () => {
    const made = await sharedPass("low");
    const view = northPole(2_000_000);
    const before = made.engine.writes.some((w) => w.buffer === "terrain instances");
    made.pass.frame({ ...frameOf(view, 0), grounded: [] });
    made.lane().bakeDemand(made.pass);
    made.pass.frame({ ...frameOf(view, 16), grounded: [] });
    expect([
      DEFAULT_TERRAIN_VIEW,
      before,
      recordBytes(made.engine, "terrain instances").length > 0,
    ]).toEqual(["", false, true]);
  });
});

/** Steps `made`'s pass over `views` and bakes its demand until none is left, then steps again. */
function settledOver(made: Made, views: ReadonlyArray<TerrainPassView>): void {
  for (let n = 0; n < 60; n += 1) {
    made.pass.step({ views, grounded: [] });
    if (made.lane().bakeDemand(made.pass) === 0) {
      break;
    }
  }
  made.pass.step({ views, grounded: [] });
}

describe("each view's own annunciation in one pass (the batch review's M2; decision-r10-t16-contracts §3)", () => {
  describe("two views, one over resident patches and one over patches not yet baked", () => {
    const resident = northPole(2_000_000);
    /** An instrument over the south pole, `heightM` up, looking down. */
    const southAt = (heightM: number): TerrainView => ({
      rotation: IDENTITY_ROTATION,
      cameraM: vec3(0, 0, -(PLANET.figure.polarRadiusM + heightM)),
      orientation: lookAlong({ x: 0, y: 0, z: 1 }, { x: 1, y: 0, z: 0 }),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 640, heightPx: 360 },
    });
    const descended = southAt(50_000);

    /**
     * Each view's frame past the annunciation's onset: both settled, then the instrument descended
     * from 1,000 km to 50 km, its finer patches not yet baked and drawn by their ancestors, the
     * primary's all resident.
     */
    async function framesOfBoth(): Promise<{
      readonly primary: TerrainViewFrame;
      readonly instrument: TerrainViewFrame;
    }> {
      const made = await sharedPass("high");
      settledOver(made, [
        passView(PRIMARY, resident),
        passView(INSTRUMENT, southAt(1_000_000), SECONDARY_VIEW_WEIGHT),
      ]);
      made.pass.step({
        views: [
          passView(PRIMARY, resident),
          passView(INSTRUMENT, descended, SECONDARY_VIEW_WEIGHT),
        ],
        grounded: [],
      });
      made.pass.drawFor(PRIMARY, frameOf(resident, 0));
      made.pass.drawFor(INSTRUMENT, frameOf(descended, 0));
      return {
        primary: made.pass.drawFor(PRIMARY, frameOf(resident, 300)),
        instrument: made.pass.drawFor(INSTRUMENT, frameOf(descended, 300)),
      };
    }

    it("announces TERRAIN: STREAMING in the view whose patches wait, once debounced", async () => {
      expect((await framesOfBoth()).instrument.annunciation).toBe("TERRAIN: STREAMING");
    });

    it("announces nothing in the view whose patches are resident", async () => {
      expect((await framesOfBoth()).primary.annunciation).toBeNull();
    });
  });

  describe("a budget that binds by an excess between the views' weights (limitExcess > w)", () => {
    // 1.5 km up, 69° from straight down, at 1080p: the instrument alone wants more patches than
    // the high setting's budget holds, the primary 2,000 km up few.
    const steep: TerrainView = {
      ...northPole(1_500),
      orientation: lookAlong(
        { x: Math.sin(1.2), y: 0, z: -Math.cos(1.2) },
        { x: Math.cos(1.2), y: 0, z: Math.sin(1.2) },
      ),
      viewport: { widthPx: 1920, heightPx: 1080 },
    };
    const high = northPole(2_000_000);

    async function settledBoth(): Promise<Made> {
      const made = await sharedPass("high");
      settledOver(made, [
        passView(PRIMARY, high),
        passView(INSTRUMENT, steep, SECONDARY_VIEW_WEIGHT),
      ]);
      return made;
    }

    it("is limited by an excess above the instrument's weight and at most the primary's", async () => {
      const selection = (await settledBoth()).pass.lastStep?.selection;
      expect(
        selection !== undefined &&
          selection.limited &&
          selection.limitExcess > SECONDARY_VIEW_WEIGHT &&
          selection.limitExcess <= PRIMARY_VIEW_WEIGHT,
      ).toBe(true);
    });

    it("limits the instrument's detail, whose τ′ the excess raises", async () => {
      const made = await settledBoth();
      expect(made.pass.drawFor(INSTRUMENT, frameOf(steep, 0)).conditions.detailLimited).toBe(true);
    });

    it("leaves the primary's detail unlimited, its τ′ its τ", async () => {
      const made = await settledBoth();
      expect(made.pass.drawFor(PRIMARY, frameOf(high, 0)).conditions.detailLimited).toBe(false);
    });
  });

  describe("a contact over unsurveyed ground (decision-r10-t16-contracts §2)", () => {
    const survey: TerrainSurveyLookup = { surveyedUnder: (key) => key.face === 5, version: 1 };
    const ground = datumGroundUnder(PLANET.figure);
    const up = normalise(vec3(0.1, 0.2, 1));
    const foot = ground(scale(up, PLANET.figure.polarRadiusM)).groundPointM;
    // A craft 900 m over the north face, unsurveyed, and closing.
    const contact = craftContact(
      add(foot, scale(up, 900)),
      scale(up, -5),
      10.3,
      ground,
      finestPatchSizeM(PLANET),
    );
    const view = northPole(2_000_000);

    async function stepped(): Promise<{ readonly made: Made; readonly frame: TerrainViewFrame }> {
      const made = await sharedPass("high", { survey });
      made.pass.step({
        views: [passView(PRIMARY, view)],
        grounded: contact === null ? [] : [contact],
      });
      made.pass.drawFor(PRIMARY, frameOf(view, 0));
      return { made, frame: made.pass.drawFor(PRIMARY, frameOf(view, 2_000)) };
    }

    it("is a contact, closing below 1 km", () => {
      expect(contact).not.toBeNull();
    });

    it("demands nothing", async () => {
      expect((await stepped()).made.lane().demand).toEqual([]);
    });

    it("draws nothing", async () => {
      expect((await stepped()).frame.draw).toBeNull();
    });

    it("leaves its view's annunciation clear", async () => {
      expect((await stepped()).frame.annunciation).toBeNull();
    });
  });
});
