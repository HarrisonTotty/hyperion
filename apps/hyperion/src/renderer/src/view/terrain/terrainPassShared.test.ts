/**
 * A body's terrain pass shared by the views that hand it over (plan R10, R10.T16.b): one
 * selection a step over every view, each view's own draw, the level table, R10's rebake rule and
 * the gate it holds the body's mask behind, the survey, the contacts, the fault and the releases,
 * over a scripted pool and R01's counting engine.
 */

import { describe, expect, it } from "vitest";

import { normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lawFor } from "../appearance/phase";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import type { MemoryCategory } from "../engine/memory";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";
import { TERRAIN_NOT_AVAILABLE } from "./annunciation";
import { allocationTally } from "./gpu/allocationTally";
import { INSTANCE_RECORD_BYTES } from "./gpu/uniforms";
import { craftContact, datumGroundUnder, finestPatchSizeM } from "./grounded";
import { patchKeyString, type PatchKey } from "./patchKey";
import { type PlanetGeometry, planetGeometry } from "./planet";
import { PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "./priority";
import { type PatchRequest, type SelectionInput, selectPatches } from "./select";
import { RESELECT_MOVE_FRACTION } from "./selectionTolerance";
import type { TerrainLighting } from "./terrainMaterial";
import {
  morphRangeM,
  TerrainPass,
  type TerrainPassOptions,
  type TerrainPassView,
  type TerrainPool,
  type TerrainSurveyLookup,
  type TerrainView,
  type TerrainViewFrameInput,
} from "./terrainPass";
import {
  type BakedPatch,
  type BakeSettings,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

const PLANET: PlanetGeometry = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** The primary's and an instrument's names, as `VIEW` gives them. */
const PRIMARY = "view";
const INSTRUMENT = "instrument 1";

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
    const n = pass.layout.atlas.samplesPerSide;
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
function passView(id: string, view: TerrainView, weight = PRIMARY_VIEW_WEIGHT): TerrainPassView {
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
  extra: Partial<Pick<TerrainPassOptions, "source" | "survey" | "onSelect">> = {},
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

  it("tells a block at once where the rule names nothing, and once its rebakes are given up", async () => {
    const { made } = await streamed();
    let settled = 0;
    made.pass.fieldChanged(
      { newlySurveyed: Uint32Array.of(1, 2), newlyHeld: new Uint32Array() },
      () => {
        settled += 1;
      },
    );
    expect(settled).toBe(1);
    const key = made.pass.lastStep?.drawSet.patches[0]?.patch.key;
    if (key === undefined) {
      throw new Error("nothing is drawn");
    }
    made.lane().layersOf.set(patchKeyString(key), layers(false));
    made.lane().deliver(made.pass, key);
    made.pass.fieldChanged(
      { newlySurveyed: Uint32Array.of(1, 2), newlyHeld: new Uint32Array() },
      () => {
        settled += 1;
      },
    );
    expect(settled).toBe(1);
    made.lane().fail(key);
    expect(settled).toBe(2);
  });

  it("tells every block waiting on its rebakes when it is disposed or rebuilt", async () => {
    const { made } = await streamed();
    const key = made.pass.lastStep?.drawSet.patches[0]?.patch.key;
    if (key === undefined) {
      throw new Error("nothing is drawn");
    }
    made.lane().layersOf.set(patchKeyString(key), layers(false));
    made.lane().deliver(made.pass, key);
    let settled = 0;
    const tell = (): void => {
      settled += 1;
    };
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      tell,
    );
    made.engine.restore();
    expect(settled).toBe(1);
    made.pass.dispose();
    made.pass.fieldChanged(
      { newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() },
      tell,
    );
    expect(settled).toBe(2);
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

  it("forces the region under a craft at 900 m and closing, and nothing for one at 2 km receding", async () => {
    const patchM = finestPatchSizeM(PLANET);
    const closing = craftContact(above(900), scale(up, -5), 10.3, ground, patchM);
    const receding = craftContact(above(2_000), scale(up, 5), 10.3, ground, patchM);
    expect(receding).toBeNull();
    const made = await sharedPass("high");
    const view = northPole(2_000_000);
    const forced = (grounded: ReturnType<typeof craftContact>[]): number =>
      [
        ...made.pass
          .step({ views: [passView(PRIMARY, view)], grounded: grounded.flatMap((c) => c ?? []) })
          .selection.patches.values(),
      ].filter((p) => p.forced).length;
    expect(forced([closing])).toBeGreaterThan(0);
    expect(forced([receding])).toBe(0);
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
