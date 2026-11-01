import { afterEach, describe, expect, it, vi } from "vitest";

import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import { bindingStages } from "../../test/wgslStages";
import { materialBindings, storageBuffersPerStage } from "../engine/webgpu/materials";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION, rotation3FromRows } from "../coords/rotation";
import { scale, sub, type Vec3 } from "../../geometry/vec3";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import {
  GEOMETRY_BEFORE_R14,
  SETTINGS,
  TERRAIN_SETTINGS,
  terrainSettingsFor,
} from "../quality/qualitySetting";
import { terrainMaterialSpec } from "./gpu/material";
import { DEFAULT_TERRAIN_VIEW, LIT_INTERIOR_INDICES, normalsAtlasOf } from "./gpu/resources";
import { CLASS_LAWS_BYTES, terrainSlotLayout } from "./slotLayout";
import { terrainWireMaterialSpec } from "./terrainWire";
import { distanceToBoxM, relativeBounds } from "./bounds";
import type { DrawSet } from "./cache";
import { vertexDir } from "./cube";
import { finestPatchSizeM } from "./grounded";
import { INSTANCE_RECORD_BYTES, NO_PARENT_SLOT, subPatchOf } from "./gpu/uniforms";
import {
  LOOKAHEAD_SAMPLES,
  LOOKAHEAD_SLOW_EVERY,
  lookaheadInput,
  lookaheadSeconds,
} from "./lookahead";
import {
  type Edge,
  EDGES,
  edgeNeighbour,
  parentKey,
  type PatchKey,
  patchKeyString,
} from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import {
  type PatchRequest,
  screenSpaceErrorPx,
  type Selection,
  type SelectionInput,
  selectionErrorM,
  selectPatches,
} from "./select";
import {
  RESELECT_MOVE_FRACTION,
  SELECTION_MARGIN,
  selectionTolerancePx,
} from "./selectionTolerance";
import { lawFor } from "../appearance/phase";
import { createAirlessTables } from "../atmosphere/surfaceLighting";
import {
  packTerrainLighting,
  type TerrainLighting,
  terrainDepthMaterialSpec,
  terrainLitEmissiveInteriorMaterialSpec,
  terrainLitEmissiveMaterialSpec,
  terrainLitInteriorMaterialSpec,
  terrainLitMaterialSpec,
} from "./terrainMaterial";
import { microtasks } from "../../test/opticsFakes";
import { lavaWorldSurface } from "../../test/thermalSurfaces";
import { surfaceTemperatureFieldOf } from "../atmosphere/thermal";
import {
  effectiveTauPx,
  morphRangeM,
  SELECT_MEASURE,
  type TerrainFrame,
  type TerrainFrameInput,
  type TerrainPassOptions,
  TerrainPass,
  type TerrainPool,
  type TerrainView,
  terrainShadingOf,
} from "./terrainPass";
import { distanceToBoxFromM } from "./viewGeometry";
import {
  type BakedPatch,
  type BakeSettings,
  NORMALS_BETWEEN,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";
import type { RefinedDraft, RefineRequest } from "./workers/pool";

/** An instance record's words (`gpu/uniforms.ts`' `INSTANCE_RECORD_BYTES`). */
const WORDS = INSTANCE_RECORD_BYTES / 4;

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** WebGPU's default `maxStorageBuffersPerShaderStage`, which the engine never raises. */
const DEFAULT_STORAGE_BUFFERS_PER_STAGE = 8;

/** A pool that records the demand and bakes on the test's command. */
class FakePool implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  /** R10's layers each bake carries: `null`, as the test planet's, unless a test sets them. */
  layers: TerrainLayers | null = null;
  /** Each key's own layers where a test gives them (R10.T16.b's rebake rule), else {@link layers}. */
  readonly layersOf = new Map<string, TerrainLayers>();
  /** Whether every worker is lost (R10.T16.b's fault). */
  unavailable = false;
  readonly #listeners = new Set<(bake: BakedPatch) => void>();
  readonly #failures = new Set<(failure: { readonly keyString: string }) => void>();
  readonly #refinedListeners = new Set<(refined: RefinedDraft) => void>();
  /** The refinements asked, in order (R10.T6.a's progressive bake). */
  readonly refines: RefineRequest[] = [];
  /** The refinements dropped, by key string, in order. */
  readonly dropped: string[] = [];
  /** The refinements asked and not yet told, as the pool counts them. */
  refiningCount = 0;
  terminated = false;

  refine(r: RefineRequest): void {
    this.refines.push(r);
  }

  dropRefinement(keyString: string): void {
    this.dropped.push(keyString);
  }

  onRefined(cb: (refined: RefinedDraft) => void): () => void {
    this.#refinedListeners.add(cb);
    return () => {
      this.#refinedListeners.delete(cb);
    };
  }

  /** Tells the pass `refined`, as the pool does a refinement's answer. */
  refined(refined: RefinedDraft): void {
    for (const listener of this.#refinedListeners) {
      listener(refined);
    }
  }

  onFailed(cb: (failure: { readonly keyString: string }) => void): () => void {
    this.#failures.add(cb);
    return () => {
      this.#failures.delete(cb);
    };
  }

  /** Gives up the request of `key`, as the pool does a bake that failed. */
  fail(key: PatchRequest["key"]): void {
    for (const listener of this.#failures) {
      listener({ keyString: patchKeyString(key) });
    }
  }

  /** Listeners still subscribed. */
  get listeners(): number {
    return this.#listeners.size;
  }

  constructor(readonly settings: BakeSettings) {}

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  terminate(): void {
    this.terminated = true;
  }

  /** Answers every demanded patch with a flat bake at its bounds' centre. */
  bakeDemand(pass: TerrainPass): number {
    const demand = this.demand;
    for (const request of demand) {
      this.deliver(pass, request.key);
    }
    return demand.length;
  }

  deliver(
    pass: TerrainPass,
    key: PatchRequest["key"],
    normals?: { readonly fill: number; readonly drafted: boolean },
  ): void {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: { x: 1_000, y: 2_000, z: PLANET.figure.polarRadiusM },
      heights: new Float32Array(65 * 65 * 2),
      offsets: this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
      normals: new Float16Array(n * n * 2).fill(normals?.fill ?? 0),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
      originHeightM: 0,
      skirtDepthM: 1,
      layers: this.layersOf.get(patchKeyString(key)) ?? this.layers,
      ...(normals?.drafted === true ? { drafted: true } : {}),
    };
    for (const listener of this.#listeners) {
      listener(bake);
    }
  }
}

/** A camera `heightM` above the north pole, `tiltRad` from straight down towards +x. */
function tilted(heightM: number, tiltRad: number): TerrainView {
  return {
    ...northPole(heightM),
    orientation: lookAlong(
      { x: Math.sin(tiltRad), y: 0, z: -Math.cos(tiltRad) },
      { x: Math.cos(tiltRad), y: 0, z: Math.sin(tiltRad) },
    ),
  };
}

/** A camera 1,000 km above the equator over `dir`, looking straight down with `up` up. */
function equatorCamera(dir: Vec3, up: Vec3): TerrainView {
  const r = PLANET.figure.equatorialRadiusM + 1_000_000;
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: { x: dir.x * r, y: dir.y * r, z: dir.z * r },
    orientation: lookAlong({ x: -dir.x, y: -dir.y, z: -dir.z }, up),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 640, heightPx: 360 },
  };
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

function inputAt(view: TerrainView, nowMs = 0): TerrainFrameInput {
  return {
    view,
    grounded: [],
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    nowMs,
  };
}

async function passOn(
  setting: "high" | "low",
  extra: Partial<
    Pick<
      TerrainPassOptions,
      "measureSelection" | "onResident" | "onSelect" | "terrain" | "source" | "lookaheadClockMs"
    >
  > = {},
): Promise<{ pass: TerrainPass; pool: () => FakePool; engine: CountingRenderEngine }> {
  const counting = await countingRenderEngine({
    maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
    maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  });
  const pools: FakePool[] = [];
  const { source = { kind: "test-planet", ridges: "off" }, ...rest } = extra;
  const pass = new TerrainPass({
    engine: counting,
    setting,
    planet: PLANET,
    source,
    createPool: (bake) => {
      const pool = new FakePool(bake);
      pools.push(pool);
      return pool;
    },
    ...rest,
  });
  await pass.ready();
  return {
    pass,
    engine: counting,
    pool: () => {
      const pool = pools.at(-1);
      if (pool === undefined) {
        throw new Error("no pool was made");
      }
      return pool;
    },
  };
}

/** The slots of the instance records last written into `buffer`, the edge patches' by default. */
function writtenSlots(engine: CountingRenderEngine, buffer = "terrain instances"): number[] {
  const write = engine.writes.findLast((w) => w.buffer === buffer);
  const u = new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
  const slots: number[] = [];
  for (let i = 0; i < u.length / (INSTANCE_RECORD_BYTES / 4); i += 1) {
    slots.push(u[i * WORDS + 3] ?? -1);
  }
  return slots;
}

afterEach(() => {
  performance.clearMeasures(SELECT_MEASURE);
});

describe("the terrain pass", () => {
  it("submits one instanced draw whose instances are the drawn patches' slots", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    const frame = pass.frame(inputAt(view));
    expect(frame.draws.at(-1)?.indirect?.buffer.name).toBe("terrain indirect");
    expect(writtenSlots(engine)).toEqual(
      Array.from(frame.drawSet.slots.subarray(0, frame.drawSet.count)),
    );
    expect(frame.drawSet.patches.length).toBeGreaterThan(0);
  });

  it("draws no instance for a resident patch that is culled", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    // A level-3 patch on the far side of the planet (face 5 is −z), resident but not selected.
    const far = { face: 5, level: 3, i: 4, j: 4 } as const;
    pool().deliver(pass, far);
    const frame = pass.frame(inputAt(view));
    expect(frame.selection.patches.has(patchKeyString(far))).toBe(false);
    const drawnKeys = frame.drawSet.patches.map((p) => p.keyString);
    expect(drawnKeys).not.toContain(patchKeyString(far));
    expect(writtenSlots(engine)).toHaveLength(frame.drawSet.patches.length);
  });

  it("draws a stand-in with its own level's morph range", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    // The roots and face 2's four level-1 patches are baked; face 2 (+z) is under the camera, so
    // its selected patches are stood in for by those level-1 patches.
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      pool().deliver(pass, { face, level: 0, i: 0, j: 0 });
    }
    for (const [i, j] of [
      [0, 0],
      [1, 0],
      [0, 1],
      [1, 1],
    ] as const) {
      pool().deliver(pass, { face: 2, level: 1, i, j });
    }
    const frame = pass.frame(inputAt(view));
    const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
    const f = new Float32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
    const levelOne = morphRangeM(PLANET, 1, {
      camera: { positionM: view.cameraM, orientation: view.orientation },
      fovXRad: view.fovXRad,
      viewport: view.viewport,
      weight: 1,
      tauPx: TERRAIN_SETTINGS.high.tauPx,
    });
    expect(levelOne[1]).toBeGreaterThan(levelOne[0]);
    const standIns = frame.drawSet.patches
      .map((drawn, i) => ({ drawn, i }))
      .filter(({ drawn }) => drawn.standIn && drawn.patch.key.level === 1);
    expect(standIns.length).toBeGreaterThan(0);
    for (const { i } of standIns) {
      expect([f[i * WORDS + 4], f[i * WORDS + 5]]).toEqual(levelOne.map((m) => Math.fround(m)));
    }
  });

  it("writes each part a stand-in draws as its slot, with the part's word, and a whole patch's with none", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      pool().deliver(pass, { face, level: 0, i: 0, j: 0 });
    }
    const frame = pass.frame(inputAt(view));
    const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
    const u = new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
    const written = frame.drawSet.patches.map(({ patch, within }, i) => [
      u[i * WORDS + 3] === patch.slot,
      u[i * WORDS + 8],
      within === null ? 0 : subPatchOf(patch.key, within),
    ]);
    // Only the roots are resident: every selected patch is a root's part.
    expect(frame.drawSet.patches.every(({ within }) => within !== null)).toBe(true);
    expect(written.every(([slot, word, wanted]) => slot === true && word === wanted)).toBe(true);
    expect(written.some(([, word]) => word !== 0)).toBe(true);
  });

  it("makes no GPU object after warm-up, frame after frame", async () => {
    const { pass, pool, engine } = await passOn("low");
    const view = northPole(500_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    pass.frame(inputAt(view, 16));
    engine.resetCounts();
    for (let n = 2; n < 30; n += 1) {
      pool().bakeDemand(pass);
      pass.frame(inputAt(view, 16 * n));
    }
    const { buffers, textures, meshes, materials, renderTargets } = engine.counts;
    expect([buffers, textures, meshes, materials, renderTargets]).toEqual([0, 0, 0, 0, 0]);
  });

  it("records no selection span unless asked to", async () => {
    const { pass } = await passOn("low");
    pass.frame(inputAt(northPole(1_000_000)));
    expect(performance.getEntriesByName(SELECT_MEASURE)).toHaveLength(0);
  });

  it("records a span for each selection when asked to", async () => {
    const { pass } = await passOn("low", { measureSelection: true });
    pass.frame(inputAt(northPole(1_000_000)));
    expect(performance.getEntriesByName(SELECT_MEASURE)).toHaveLength(1);
  });

  it("tells each selection's input and result, and only when it selects", async () => {
    const seen: Array<[SelectionInput, Selection]> = [];
    const { pass } = await passOn("low", {
      onSelect: (input, selection) => seen.push([input, selection]),
    });
    const first = pass.frame(inputAt(northPole(1_000_000)));
    pass.frame(inputAt(northPole(1_000_000), 16));
    expect(seen).toHaveLength(1);
    expect(seen[0]?.[1]).toBe(first.selection);
    expect(seen[0]?.[0].maxPatches).toBe(pass.maxPatches);
  });

  it.each(["high", "low"] as const)(
    "selects the %s setting at τ ÷ (1 + SELECTION_MARGIN), the demand record's τ_sel",
    async (setting) => {
      const seen: SelectionInput[] = [];
      const { pass } = await passOn(setting, { onSelect: (input) => seen.push(input) });
      pass.frame(inputAt(northPole(1_000_000)));
      const tauPx = seen[0]?.views[0]?.tauPx;
      expect(tauPx).toBe(TERRAIN_SETTINGS[setting].tauPx / (1 + SELECTION_MARGIN));
      expect(tauPx).toBe(selectionTolerancePx(TERRAIN_SETTINGS[setting].tauPx));
    },
  );

  it("tells of each patch it makes resident", async () => {
    const resident: string[] = [];
    const { pass, pool } = await passOn("low", {
      onResident: (key) => resident.push(patchKeyString(key)),
    });
    pass.frame(inputAt(northPole(1_000_000)));
    const demanded = pool().demand.map((r) => patchKeyString(r.key));
    pool().bakeDemand(pass);
    expect(resident).toEqual(demanded);
  });

  it("draws the spike's terrain variant: its vertex path, its normals and its slots", async () => {
    const terrain = terrainSettingsFor("high", { vertexPath: "face-differences", normals: "mesh" });
    const { pass, pool } = await passOn("high", { terrain });
    expect([pool().settings.vertexPath, pool().settings.normals]).toEqual([
      "face-differences",
      "mesh",
    ]);
    expect(pass.layout.slots.slotCount).toBe(terrainSlotLayout(terrain).slotCount);
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    const frame = pass.frame(inputAt(view));
    expect(frame.draws.at(-1)?.material.name).toBe(terrainMaterialSpec("face-differences").name);
  });

  it("keeps its variant through a device restore", async () => {
    const terrain = terrainSettingsFor("high", { vertexPath: "face-differences", normals: "mesh" });
    const { pass, pool, engine } = await passOn("high", { terrain });
    const first = pool();
    engine.restore();
    expect(pool()).not.toBe(first);
    expect([pool().settings.vertexPath, pool().settings.normals]).toEqual([
      "face-differences",
      "mesh",
    ]);
    expect(pass.layout.vertexPath).toBe("face-differences");
  });

  it("budgets half the high setting's BakedOffsets slots for selection", async () => {
    expect((await passOn("high")).pass.maxPatches).toBe(981);
  });

  it("budgets half the low setting's slots for selection", async () => {
    expect((await passOn("low")).pass.maxPatches).toBe(648);
  });

  it("selects no more patches than its budget, and says the budget bound", async () => {
    // At BUDGET_VIEW the demand is baked until it empties, every frame's selection counted.
    const { maxPatches, sizes, frame } = await streamedUnderBudget();
    for (const size of sizes) {
      expect(size).toBeLessThanOrEqual(maxPatches);
    }
    expect(frame.selection.limited).toBe(true);
    expect(frame.conditions.detailLimited).toBe(true);
  });

  it("does not select again, and draws the same slots, while nothing changes", async () => {
    const { pass, pool } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    const first = pass.frame(inputAt(view));
    const slots = Array.from(first.drawSet.slots.subarray(0, first.drawSet.count));
    const second = pass.frame(inputAt(view, 16));
    expect(second.reselected).toBe(false);
    expect(Array.from(second.drawSet.slots.subarray(0, second.drawSet.count))).toEqual(slots);
  });

  it("rewrites one draw set in place across selections", async () => {
    const { pass, pool } = await passOn("low");
    const view = northPole(1_000_000);
    const first = pass.frame(inputAt(view));
    const { patches, slots } = first.drawSet;
    pool().bakeDemand(pass);
    const second = pass.frame(inputAt(view, 16));
    expect(second.reselected).toBe(true);
    expect(second.drawSet).toBe(first.drawSet);
    expect(second.drawSet.patches).toBe(patches);
    expect(second.drawSet.slots).toBe(slots);
    expect(second.drawSet.count).toBeGreaterThan(0);
  });

  it("requests a forced region no view sees and keeps it resident, but draws none of it", async () => {
    const { pass, pool, engine } = await passOn("low");
    const view = northPole(1_000_000);
    // A craft on the south pole, on the far side from the camera.
    const grounded = [{ positionM: { x: 0, y: 0, z: -PLANET.figure.polarRadiusM }, radiusM: 10 }];
    let frame = pass.frame({ ...inputAt(view), grounded });
    const baked = new Set<string>();
    for (let n = 1; n < 8 && pool().demand.length > 0; n += 1) {
      for (const request of pool().demand) {
        baked.add(patchKeyString(request.key));
      }
      pool().bakeDemand(pass);
      frame = pass.frame({ ...inputAt(view, 16 * n), grounded });
    }
    const unseen = [...frame.selection.patches.values()].filter((p) => !p.seen);
    expect(unseen.length).toBeGreaterThan(0);
    const unseenKeys = new Set(unseen.map((p) => patchKeyString(p.key)));
    expect([...unseenKeys].every((k) => baked.has(k))).toBe(true);
    // The demand leaves out resident patches, so none of the region has been evicted.
    expect(pool().demand.filter((r) => unseenKeys.has(patchKeyString(r.key)))).toEqual([]);
    const drawnKeys = frame.drawSet.patches.map((p) => p.keyString);
    expect(drawnKeys.filter((k) => unseenKeys.has(k))).toEqual([]);
    expect(writtenSlots(engine)).toHaveLength(frame.drawSet.count);
  });

  it("draws a turned body's patches at R · origin less the camera, with R as its rotation", async () => {
    const { pass, pool, engine } = await passOn("low");
    // The body turned 90° about its pole: body-fixed x lies along the frame's y.
    const rotation = rotation3FromRows([
      { x: 0, y: -1, z: 0 },
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 0, z: 1 },
    ]);
    const view = { ...northPole(1_000_000), rotation };
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    const frame = pass.frame(inputAt(view));
    const drawn = frame.drawSet.patches[0];
    if (drawn === undefined) {
      throw new Error("nothing was drawn");
    }
    const o = drawn.patch.originM;
    const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
    const f = new Float32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
    expect([f[0], f[1], f[2]]).toEqual(
      [-o.y - view.cameraM.x, o.x - view.cameraM.y, o.z - view.cameraM.z].map((v) =>
        Math.fround(v),
      ),
    );
    // Column-major: the first column is R's first column, (0, 1, 0).
    expect(Array.from(frame.draws.at(-1)?.uniforms["bodyRotation"] ?? []).slice(0, 4)).toEqual([
      0, 1, 0, 0,
    ]);
  });

  it("selects for a turned body as for the same camera given body-fixed", async () => {
    const turned = await passOn("low");
    const fixed = await passOn("low");
    const rotation = rotation3FromRows([
      { x: 0, y: -1, z: 0 },
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 0, z: 1 },
    ]);
    // A camera over the equator at body-fixed +x, which the turned body carries to the frame's +y.
    const bodyFixed = equatorCamera({ x: 1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 });
    const inFrame: TerrainView = {
      ...equatorCamera({ x: 0, y: 1, z: 0 }, { x: 0, y: 0, z: 1 }),
      rotation,
    };
    const a = turned.pass.frame(inputAt(inFrame)).selection;
    const b = fixed.pass.frame(inputAt(bodyFixed)).selection;
    expect([...a.patches.keys()].toSorted()).toEqual([...b.patches.keys()].toSorted());
  });

  it("never writes more instances than slots", async () => {
    const { pass, pool, engine } = await passOn("low");
    const view = northPole(200_000);
    for (let n = 0; n < 5; n += 1) {
      pass.frame(inputAt(view, 16 * n));
      pool().bakeDemand(pass);
    }
    expect(writtenSlots(engine).length).toBeLessThanOrEqual(pass.layout.slots.slotCount);
  });
});

describe("the terrain pass's selection cadence", () => {
  it("does not select again at the same pose", async () => {
    const { pass } = await passOn("low");
    const view = northPole(1_000_000);
    expect(pass.frame(inputAt(view)).reselected).toBe(true);
    expect(pass.frame(inputAt(view)).reselected).toBe(false);
  });

  it("selects again once the camera moves past its fraction of the nearest patch", async () => {
    const { pass, pool } = await passOn("low");
    const heightM = 1_000_000;
    // Streamed in first: selection descends as bakes land (R05.T7's streaming gate).
    for (let n = 0; n < 20; n += 1) {
      pass.frame(inputAt(northPole(heightM), 16 * n));
      pool().bakeDemand(pass);
    }
    pass.frame(inputAt(northPole(heightM), 16 * 20));
    // The nearest selected patch is at least the height's distance below the camera, less the
    // relief, so a move of a thousandth of it is well within the fraction.
    expect(pass.frame(inputAt(northPole(heightM * 0.999))).reselected).toBe(false);
    expect(pass.frame(inputAt(northPole(heightM * (1 - 2 * SELECTION_MARGIN)))).reselected).toBe(
      true,
    );
  });

  it("does not select a low camera again for a short move once streamed", async () => {
    const { pass, pool } = await passOn("low");
    const heightM = 1_500;
    pass.frame(inputAt(northPole(heightM)));
    for (let n = 1; n < 40 && pool().bakeDemand(pass) > 0; n += 1) {
      pass.frame(inputAt(northPole(heightM), 16 * n));
    }
    pass.frame(inputAt(northPole(heightM), 16 * 41));
    // 10 m is far below m ÷ (1 + m) of the 1.5 km to the nearest box (about 136 m), and above it
    // of one finest patch (about 1.9 m), the rule's floor. The fake pool's flat bakes give spheres
    // that hold no camera, so this does not tell the box from the sphere; the captures exercise
    // that.
    expect(pass.frame(inputAt(northPole(heightM - 10), 16 * 42)).reselected).toBe(false);
  });

  it("holds every drawn leaf within τ up to a move of m ÷ (1 + m) of the nearest box", async () => {
    const { pass, pool } = await passOn("low");
    const heightM = 1_500;
    const frame = streamed(pass, pool, northPole(heightM));
    expect([frame.selection.limited, frame.drawSet.standingIn, frame.drawSet.missing]).toEqual([
      false,
      0,
      0,
    ]);
    const dMinM = nearestBoxM(frame.selection, northPole(heightM).cameraM);
    // Just short of the move that selects again: the farthest the pass draws this selection from.
    const farthest = northPole(heightM - RESELECT_MOVE_FRACTION * (1 - 1e-6) * dMinM);
    expect(drawnErrorPx(frame.selection, northPole(heightM))).toBeLessThanOrEqual(
      selectionTolerancePx(TERRAIN_SETTINGS.low.tauPx),
    );
    expect(pass.frame(inputAt(farthest, 16 * 100)).reselected).toBe(false);
    expect(drawnErrorPx(frame.selection, farthest)).toBeLessThanOrEqual(TERRAIN_SETTINGS.low.tauPx);
  });

  it("selects again on a move past m ÷ (1 + m) of the nearest box, short of m", async () => {
    const { pass, pool } = await passOn("low");
    const heightM = 1_500;
    const frame = streamed(pass, pool, northPole(heightM));
    const dMinM = nearestBoxM(frame.selection, northPole(heightM).cameraM);
    // Halfway between the two: the former rule, m of d_min, drew the old selection on from here.
    const fraction = (RESELECT_MOVE_FRACTION + SELECTION_MARGIN) / 2;
    expect(pass.frame(inputAt(northPole(heightM - fraction * dMinM), 16 * 100)).reselected).toBe(
      true,
    );
  });

  it("does not select again for a turn of less than a pixel", async () => {
    const { pass } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    const pixel = view.fovXRad / view.viewport.widthPx;
    const turned: TerrainView = {
      ...view,
      orientation: lookAlong({ x: Math.tan(0.4 * pixel), y: 0, z: -1 }, { x: 1, y: 0, z: 0 }),
    };
    expect(pass.frame(inputAt(turned)).reselected).toBe(false);
  });

  it("selects again when the camera rolls", async () => {
    const { pass } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    const rolled: TerrainView = {
      ...view,
      orientation: lookAlong({ x: 0, y: 0, z: -1 }, { x: Math.cos(0.1), y: Math.sin(0.1), z: 0 }),
    };
    expect(pass.frame(inputAt(rolled)).reselected).toBe(true);
  });

  it("selects again when the field of view changes", async () => {
    const { pass } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    expect(pass.frame(inputAt({ ...view, fovXRad: Math.PI / 4 })).reselected).toBe(true);
  });

  it("selects again when a contact changed in place", async () => {
    const { pass } = await passOn("low");
    const view = northPole(1_000_000);
    const grounded = [{ positionM: { x: 0, y: 0, z: 6_356_752 }, radiusM: 10 }];
    pass.frame({ ...inputAt(view), grounded });
    const moved = [{ positionM: { x: 0, y: 0, z: 6_356_752 }, radiusM: 20 }];
    expect(pass.frame({ ...inputAt(view), grounded: moved }).reselected).toBe(true);
  });

  it("selects again when the camera turns by more than a pixel", async () => {
    const { pass } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    const turned: TerrainView = {
      ...view,
      orientation: lookAlong({ x: 0.01, y: 0, z: -1 }, { x: 1, y: 0, z: 0 }),
    };
    expect(pass.frame(inputAt(turned)).reselected).toBe(true);
  });

  it("selects again when a bake lands", async () => {
    const { pass, pool } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    pool().deliver(pass, { face: 2, level: 0, i: 0, j: 0 });
    expect(pass.frame(inputAt(view)).reselected).toBe(true);
  });
});

describe("the terrain pass's annunciation", () => {
  it("says DETAIL LIMITED on the low setting once everything is resident", async () => {
    const { pass, pool } = await passOn("low");
    const view = northPole(1_000_000);
    // Demand descends breadth-first, a level a round: bake until nothing stands in.
    let t = 0;
    let frame = pass.frame(inputAt(view, t));
    for (let round = 0; round < 40 && frame.conditions.streaming; round += 1) {
      pool().bakeDemand(pass);
      t += 16;
      frame = pass.frame(inputAt(view, t));
    }
    expect(frame.conditions).toEqual({ streaming: false, detailLimited: true });
    // STREAMING clears 1 s after the last stand-in; DETAIL LIMITED stays.
    expect(pass.frame(inputAt(view, t + 1_500)).annunciation).toBe("TERRAIN: DETAIL LIMITED");
  });

  it("says STREAMING while a selected patch stands in", async () => {
    const { pass } = await passOn("high");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view, 0));
    const frame = pass.frame(inputAt(view, 400));
    expect(frame.conditions.streaming).toBe(true);
    expect(frame.annunciation).toBe("TERRAIN: STREAMING");
  });
});

describe("the morph bands", () => {
  const view = {
    camera: {
      positionM: { x: 0, y: 0, z: 7e6 },
      orientation: lookAlong({ x: 0, y: 0, z: -1 }, { x: 1, y: 0, z: 0 }),
    },
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
    weight: 1,
    tauPx: 1,
  };

  it("are empty at level 0, which has no parent", () => {
    expect(morphRangeM(PLANET, 0, view)).toEqual([0, 0]);
  });

  it("start from 0.7 of the parent's distance at the finest level, whose own error is zero", () => {
    const [start, end] = morphRangeM(PLANET, PLANET.finestLevel, view);
    expect(end).toBeGreaterThan(0);
    expect(start).toBeCloseTo(0.7 * end, 9);
  });
});

describe("the morph bands' tolerance", () => {
  it.each([
    { name: "τ with no budget binding", tauPx: 1, limitExcess: 0, weight: 1, want: 1 },
    {
      name: "τ in a secondary view with no budget binding",
      tauPx: 4,
      limitExcess: 0,
      weight: 0.25,
      want: 4,
    },
    {
      name: "τ × the excess in the primary view",
      tauPx: 1,
      limitExcess: 2.5,
      weight: 1,
      want: 2.5,
    },
    {
      name: "τ × the excess ÷ w in a secondary view",
      tauPx: 4,
      limitExcess: 1,
      weight: 0.25,
      want: 16,
    },
    {
      name: "τ in the primary view where only a secondary wanted the refused split",
      tauPx: 1,
      limitExcess: 0.9,
      weight: 1,
      want: 1,
    },
  ])("is $name", ({ tauPx, limitExcess, weight, want }) => {
    expect(effectiveTauPx(tauPx, limitExcess, weight)).toBe(want);
  });
});

/**
 * Frames at `view` until the demand is baked out, and returns the last frame. Selection descends
 * as bakes land (R05.T7's streaming gate).
 */
function streamed(pass: TerrainPass, pool: () => FakePool, view: TerrainView): TerrainFrame {
  let frame = pass.frame(inputAt(view));
  for (let n = 1; n < 60 && pool().bakeDemand(pass) > 0; n += 1) {
    frame = pass.frame(inputAt(view, 16 * n));
  }
  return frame;
}

/**
 * 1.5 km up, 69° from straight down, at 1080p: even under the fake pool's flat bakes, whose ranges
 * are tight, the high setting wants several thousand patches here, beyond its budget of 981.
 */
const BUDGET_VIEW: TerrainView = {
  ...tilted(1_500, 1.2),
  viewport: { widthPx: 1920, heightPx: 1080 },
};

/** The high setting's pass streamed at {@link BUDGET_VIEW}, as {@link streamed} streams it. */
interface BudgetStream {
  /** The pass's budget, patches. */
  readonly maxPatches: number;
  /** Each frame's selected patches, the first frame's first. */
  readonly sizes: ReadonlyArray<number>;
  /** The last frame, whose draw set no later frame rewrites. */
  readonly frame: TerrainFrame;
  /** The morph bands the last frame wrote ({@link writtenBands}). */
  readonly bands: ReadonlyMap<string, readonly [number, number]>;
}

let budgetStream: Promise<BudgetStream> | undefined;

/**
 * {@link BUDGET_VIEW}'s stream, run once for the budget's test and the morph bands' tests under
 * it, which all read the same run: every input is fixed (the setting, the view, the fake pool's
 * bakes and the frames' clock), and the run's figures are only read.
 */
function streamedUnderBudget(): Promise<BudgetStream> {
  budgetStream ??= (async (): Promise<BudgetStream> => {
    const { pass, pool, engine } = await passOn("high");
    // Selection descends as bakes land (R05.T7's streaming gate), so the demand is baked until it
    // empties.
    const sizes: number[] = [];
    let frame = pass.frame(inputAt(BUDGET_VIEW));
    sizes.push(frame.selection.patches.size);
    for (let n = 1; n < 60 && pool().bakeDemand(pass) > 0; n += 1) {
      frame = pass.frame(inputAt(BUDGET_VIEW, 16 * n));
      sizes.push(frame.selection.patches.size);
    }
    return {
      maxPatches: pass.maxPatches,
      sizes,
      frame,
      bands: writtenBands(engine, frame.drawSet),
    };
  })();
  return budgetStream;
}

/**
 * The pass's d_min: the box distance from `cameraM` to the nearest selected patch not at the
 * finest level, floored at one finest patch, metres.
 */
function nearestBoxM(selection: Selection, cameraM: Vec3): number {
  let nearestM = Number.POSITIVE_INFINITY;
  for (const patch of selection.patches.values()) {
    if (patch.key.level < PLANET.finestLevel) {
      nearestM = Math.min(nearestM, distanceToBoxM(relativeBounds(patch.bounds, cameraM)));
    }
  }
  return Math.max(nearestM, finestPatchSizeM(PLANET));
}

/**
 * The largest ρ any selected patch `view` sees draws from `view`'s camera, pixels: its level's
 * selection error at its box's distance, floored at the near plane, as selection measures it.
 */
function drawnErrorPx(selection: Selection, view: TerrainView): number {
  const selectionView = {
    camera: { positionM: view.cameraM, orientation: view.orientation },
    fovXRad: view.fovXRad,
    viewport: view.viewport,
    weight: 1,
    tauPx: TERRAIN_SETTINGS.low.tauPx,
  };
  let largestPx = 0;
  for (const patch of selection.patches.values()) {
    if (patch.seen) {
      const distanceM = Math.max(distanceToBoxFromM(patch.bounds, view.cameraM), NEAR_PLANE_M);
      largestPx = Math.max(
        largestPx,
        screenSpaceErrorPx(selectionErrorM(PLANET, patch.key.level), distanceM, selectionView),
      );
    }
  }
  return largestPx;
}

/** The morph band last written for each drawn patch, by key string, as the shader reads it. */
function writtenBands(
  engine: CountingRenderEngine,
  drawSet: DrawSet,
): Map<string, [number, number]> {
  const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
  const f = new Float32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
  const bands = new Map<string, [number, number]>();
  drawSet.patches.forEach((drawn, i) => {
    bands.set(drawn.keyString, [f[i * WORDS + 4] ?? Number.NaN, f[i * WORDS + 5] ?? Number.NaN]);
  });
  return bands;
}

/** Each drawn patch's morph band in `view`, computed at `tauPx`, as `f32`. */
function bandsAt(view: TerrainView, drawSet: DrawSet, tauPx: number): Map<string, number[]> {
  const bands = new Map<string, number[]>();
  for (const { keyString, patch } of drawSet.patches) {
    const band = morphRangeM(PLANET, patch.key.level, {
      camera: { positionM: view.cameraM, orientation: view.orientation },
      fovXRad: view.fovXRad,
      viewport: view.viewport,
      weight: 1,
      tauPx,
    });
    bands.set(
      keyString,
      band.map((m) => Math.fround(m)),
    );
  }
  return bands;
}

/** The 65 vertices along `edge` of a patch's grid: column x = 0 or 64, or row y = 0 or 64. */
function edgeVertices(edge: Edge): Array<readonly [number, number]> {
  const fixed = edge === "UMin" || edge === "VMin" ? 0 : 64;
  const column = edge === "UMin" || edge === "UMax";
  return Array.from({ length: 65 }, (_, n): readonly [number, number] =>
    column ? [fixed, n] : [n, fixed],
  );
}

/** The shader's `morphFactor` with no contact: clamp((d − start) ÷ (end − start), 0, 1). */
function morphFactor([start, end]: readonly [number, number], distanceM: number): number {
  const span = end - start;
  return span > 0 ? Math.min(Math.max((distanceM - start) / span, 0), 1) : 0;
}

describe("the morph bands under the patch budget", () => {
  // The budget test's run: the high setting wants several thousand patches here, beyond 981.
  const limitedView = BUDGET_VIEW;

  it("are at the setting's τ, bit for bit, while the budget does not bind", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    const frame = streamed(pass, pool, view);
    expect([frame.selection.limited, frame.selection.limitExcess]).toEqual([false, 0]);
    expect(frame.drawSet.count).toBeGreaterThan(0);
    expect(writtenBands(engine, frame.drawSet)).toEqual(
      bandsAt(view, frame.drawSet, TERRAIN_SETTINGS.high.tauPx),
    );
  });

  it("are at τ × max(1, the limit excess) while the budget binds", async () => {
    const { frame, bands } = await streamedUnderBudget();
    expect(frame.selection.limited).toBe(true);
    // Above 1, so that τ′ is not the setting's τ.
    expect(frame.selection.limitExcess).toBeGreaterThan(1);
    const tauPx = TERRAIN_SETTINGS.high.tauPx * frame.selection.limitExcess;
    expect(bands).toEqual(bandsAt(limitedView, frame.drawSet, tauPx));
  });

  it("keep every coarse–fine edge at morph 1 while the budget binds", async () => {
    const { frame, bands } = await streamedUnderBudget();
    expect(frame.selection.limited).toBe(true);
    // Past the bands' margin, so that bands at the setting's τ would leave edges unmorphed.
    expect(frame.selection.limitExcess).toBeGreaterThan(1 + SELECTION_MARGIN);
    // Nothing stands in, so every drawn patch is a selected leaf.
    expect(frame.drawSet.standingIn).toBe(0);
    const camera = limitedView.cameraM;
    let edges = 0;
    const unmorphed: string[] = [];
    for (const { keyString, patch } of frame.drawSet.patches) {
      const band = bands.get(keyString);
      if (band === undefined) {
        throw new Error(`no band was written for ${keyString}`);
      }
      for (const edge of EDGES) {
        // A drawn patch a level coarser across the edge: the fine side's edge vertices lie on it.
        const coarse = parentKey(edgeNeighbour(patch.key, edge));
        if (coarse === null || !bands.has(patchKeyString(coarse))) {
          continue;
        }
        edges += 1;
        for (const [x, y] of edgeVertices(edge)) {
          // The unmorphed vertex, at the fake pool's flat bake's height of 0.
          const [px, py, pz] = surfacePoint(PLANET.figure, vertexDir(patch.key, x, y), 0);
          const k = morphFactor(band, Math.hypot(px - camera.x, py - camera.y, pz - camera.z));
          if (k !== 1) {
            unmorphed.push(`${keyString} ${edge} (${x}, ${y}): ${k}`);
          }
        }
      }
    }
    expect(edges).toBeGreaterThan(0);
    expect(unmorphed).toEqual([]);
  });
});

describe("the terrain pass after a device loss", () => {
  it("ignores a late bake from the pool it replaced", async () => {
    const { pass, pool, engine } = await passOn("low");
    const view = northPole(1_000_000);
    pass.frame(inputAt(view));
    const old = pool();
    engine.restore();
    old.deliver(pass, { face: 2, level: 0, i: 0, j: 0 });
    const frame = pass.frame(inputAt(view));
    expect(frame.drawSet.patches).toEqual([]);
  });

  it("stops hearing bakes once disposed", async () => {
    const { pass, pool } = await passOn("low");
    pass.dispose();
    expect(pool().listeners).toBe(0);
  });

  it("makes a new pool and drops the old one", async () => {
    const { pass, pool, engine } = await passOn("low");
    const first = pool();
    engine.restore();
    expect(first.terminated).toBe(true);
    expect(pool()).not.toBe(first);
    pass.dispose();
    expect(pool().terminated).toBe(true);
  });
});

/** A body's layers: slot 0 regolith (law 1, L(0) 1), slot 1 unnamed, the weights all slot 0's. */
function bodyLayers(): TerrainLayers {
  const palette = new Uint8Array(16).fill(PALETTE_NONE);
  palette.set([1, 0, 2, PALETTE_NONE]);
  const weights = new Uint8Array(65 * 65 * 8);
  for (let v = 0; v < 65 * 65; v += 1) {
    weights[8 * v] = 255;
  }
  const photometry = new Float32Array(40);
  photometry.set([0.1, 0.12, 0.14, 1, 1]);
  return {
    palette,
    weights,
    surveyMask: new Uint8Array(65 * 65).fill(255),
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    photometry,
    // A horizon map of 0° everywhere, the codes 0.
    horizonNear: new Uint32Array(65 * 65 * 4),
    horizonFar: new Uint32Array(9 * 9 * 4),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/**
 * {@link bodyLayers} with one vertex, the corner (0, 0), outside the survey: its patches are edge
 * patches, drawn by the lit mesh and the material with the survey discard (plan R14, R14.T1.a).
 */
function edgeLayers(): TerrainLayers {
  const layers = bodyLayers();
  const surveyMask = layers.surveyMask.slice();
  surveyMask[0] = 0;
  return { ...layers, surveyMask };
}

/**
 * The parent slots and quadrants of the instance records last written, each quadrant without the
 * morph holds above its two bits (R10.T8.c).
 */
function writtenParents(engine: CountingRenderEngine): Array<readonly [number, number]> {
  const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
  const u = new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
  const parents: Array<readonly [number, number]> = [];
  for (let i = 0; i < u.length / (INSTANCE_RECORD_BYTES / 4); i += 1) {
    const quadrant = u[i * WORDS + 7];
    parents.push([u[i * WORDS + 6] ?? -1, quadrant === undefined ? -1 : quadrant & 3]);
  }
  return parents;
}

describe("the lit terrain of a body (R10.T10.b)", () => {
  const BODY = {
    kind: "body",
    body: "0200080020000000.0301",
    detailSeed: "617373656d626c79",
    weights: "baked",
  } as const;

  const LIGHTING = {
    lights: [],
    occluders: [],
    secondaries: [],
    elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
    exposureScale: 1e-4,
  } satisfies TerrainLighting;

  it("shades a body baked with its weights with the lit material, the wireframe's bakes with its depth and contours (R10.T11), and the test planet with R05's", async () => {
    const lit = await passOn("high", { source: BODY });
    expect(lit.pass.shading).toBe("lit");
    expect(lit.pass.layout.shading).toBe("lit");
    expect(terrainShadingOf({ ...BODY, weights: "skipped" })).toBe("wire");
    expect(terrainShadingOf({ kind: "test-planet", ridges: "on" })).toBe("spike");
    const planet = await passOn("high");
    expect(planet.pass.shading).toBe("spike");
    // R05's layout keeps its slots: the lit fields are the lit layout's alone.
    expect(planet.pass.layout.slots).toEqual(terrainSlotLayout(TERRAIN_SETTINGS.high));
    expect(lit.pass.layout.slots).toEqual(terrainSlotLayout(TERRAIN_SETTINGS.high, "lit"));
  });

  it("binds the slots' class weights and laws, the lights, the phase table and a body without air's tables", async () => {
    const { pass, pool, engine } = await passOn("low", { source: BODY });
    pool().layers = edgeLayers();
    const view = northPole(2_000_000);
    pass.frame({ ...inputAt(view), lighting: LIGHTING });
    pool().bakeDemand(pass);
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const draw = frame.draws.at(-1);
    if (draw === undefined) {
      throw new Error("the lit pass drew nothing");
    }
    expect(draw.material.name).toBe(terrainLitMaterialSpec(pass.layout.vertexPath).name);
    expect(Object.keys(draw.storageBuffers ?? {}).toSorted()).toEqual([
      "classLayers",
      "contacts",
      "heights",
      "horizonMap",
      "instances",
      "slots",
      "surveyMask",
      "terrainLights",
    ]);
    expect(draw.storageBuffers?.["horizonMap"]?.name).toBe("terrain horizon-map");
    expect(draw.storageBuffers?.["surveyMask"]?.name).toBe("terrain survey-mask");
    expect(Object.keys(draw.textures).toSorted()).toEqual([
      "atmosphereAbsorberColumns",
      "atmosphereAbsorberCurves",
      "atmosphereAbsorberPressureColumns",
      "atmosphereBelowDeck",
      "atmosphereBody",
      "atmosphereIrradiance",
      "atmosphereLastAbsorberPressureColumn",
      "atmosphereTransmittance",
      "normals",
      "phaseFactorTable",
      "shadowCascades",
    ]);
    // The low setting has no cascades: the placeholder stands for them (R10.T9.b).
    expect(draw.textures["shadowCascades"]?.name).toBe("terrain shadow placeholder");
    // The camera body-fixed, with 1 ÷ a beside it.
    expect([...(draw.uniforms["cameraBodyFixedM"] ?? [])]).toEqual(
      [0, 0, PLANET.figure.polarRadiusM + 2_000_000, 1 / PLANET.figure.equatorialRadiusM].map(
        Math.fround,
      ),
    );
    const lights = engine.writes.findLast((w) => w.buffer === "terrain lights");
    expect(lights?.data).toEqual(
      new Uint8Array(
        packTerrainLighting(LIGHTING, PLANET.figure.polarRadiusM / PLANET.figure.equatorialRadiusM)
          .buffer,
      ),
    );
    // Slot 0's class law: regolith on its law's row, after row 0, `elsewhere`; slot 1 has none.
    const laws = engine.writes.findLast(
      (w) => w.buffer === "terrain class-layers" && w.bytes === CLASS_LAWS_BYTES,
    );
    const packed = new Float32Array(laws?.data.slice().buffer ?? new ArrayBuffer(0));
    expect([...packed.subarray(0, 5)]).toEqual([0.14, 0.12, 0.1, 1, 1].map(Math.fround));
    expect(packed[8 + 4]).toBe(-1);
  });

  it("draws nothing lit until a frame gives its lights, and binds the frame's atmosphere where given", async () => {
    const { pass, pool, engine } = await passOn("low", { source: BODY });
    pool().layers = edgeLayers();
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    const unlit = pass.frame(inputAt(view));
    expect(unlit.draws.at(-1)).toBeDefined();
    expect(engine.writes.filter((w) => w.buffer === "terrain lights")).toHaveLength(1);
    const air = createAirlessTables(engine, "a body's air");
    const withAir = pass.frame({ ...inputAt(view), lighting: { ...LIGHTING, atmosphere: air } });
    expect(withAir.draws.at(-1)?.textures["atmosphereBody"]).toBe(air.body);
  });

  /** A lit pass on low over `bodyLayers`, its patches baked, and its view. */
  async function bakedLitPass(): Promise<
    Awaited<ReturnType<typeof passOn>> & { readonly view: TerrainView }
  > {
    const made = await passOn("low", { source: BODY });
    made.pool().layers = edgeLayers();
    const view = northPole(2_000_000);
    made.pass.frame({ ...inputAt(view), lighting: LIGHTING });
    made.pool().bakeDemand(made.pass);
    return { ...made, view };
  }

  /** {@link LIGHTING} carrying the lava world's ground's emission about a star over the pole. */
  const EMITTING: TerrainLighting = {
    ...LIGHTING,
    emission: {
      field: surfaceTemperatureFieldOf(lavaWorldSurface()),
      star: { x: 0, y: 0, z: 1 },
      meltClasses: [],
    },
  };

  it("draws a body whose lighting carries its ground's emission by the plain form until the emissive form is made (R08.T18.a)", async () => {
    const { pass, view } = await bakedLitPass();
    const plain = pass.frame({ ...inputAt(view), lighting: EMITTING });
    expect(plain.draws.at(-1)?.material.name).toBe(
      terrainLitMaterialSpec(pass.layout.vertexPath).name,
    );
  });

  it("draws it by the emissive form once that is made (R08.T18.a)", async () => {
    const { pass, view } = await bakedLitPass();
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    const emissive = pass.frame({ ...inputAt(view), lighting: EMITTING });
    expect(emissive.draws.at(-1)?.material.name).toBe(
      terrainLitEmissiveMaterialSpec(pass.layout.vertexPath).name,
    );
  });

  it("writes the emission's block past the lights (R08.T18.a)", async () => {
    const { pass, engine, view } = await bakedLitPass();
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    const lights = engine.writes.findLast((w) => w.buffer === "terrain lights");
    expect(lights?.data).toEqual(
      new Uint8Array(
        packTerrainLighting(EMITTING, PLANET.figure.polarRadiusM / PLANET.figure.equatorialRadiusM)
          .buffer,
      ),
    );
  });

  it("goes back to the plain form for a frame without the emission (R08.T18.a)", async () => {
    const { pass, view } = await bakedLitPass();
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    const after = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    expect(after.draws.at(-1)?.material.name).toBe(
      terrainLitMaterialSpec(pass.layout.vertexPath).name,
    );
  });

  it("asks for the emissive form again on the device a restore makes (R08.T18.a)", async () => {
    const { pass, pool, engine, view } = await bakedLitPass();
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    const name = terrainLitEmissiveMaterialSpec(pass.layout.vertexPath).name;
    const asked = (): number => engine.compiles.filter((made) => made.name === name).length;
    expect(asked()).toBe(1);
    engine.loseDevice();
    engine.restore();
    await pass.ready();
    // The new device's slots are empty until its pool bakes them again.
    pool().layers = edgeLayers();
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    pool().bakeDemand(pass);
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    expect(asked()).toBe(2);
  });

  it("reports an emissive form the device refuses, and draws on by the plain form (R08.T18.a)", async () => {
    const { pass, engine, view } = await bakedLitPass();
    const reported = vi.spyOn(console, "error").mockImplementation(() => undefined);
    engine.failMaterials((name) => name.startsWith("terrain-lit-emissive"));
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    const frame = pass.frame({ ...inputAt(view), lighting: EMITTING });
    expect(frame.draws.at(-1)?.material.name).toBe(
      terrainLitMaterialSpec(pass.layout.vertexPath).name,
    );
    expect(reported).toHaveBeenCalledWith(
      "the terrain's emissive form failed: its ground is drawn unlit by itself",
      expect.any(Error),
    );
  });

  it("binds the larger phase table once the slots' laws outgrow it", async () => {
    const { pass, pool } = await passOn("low", { source: BODY });
    pool().layers = edgeLayers();
    const view = northPole(2_000_000);
    pass.frame({ ...inputAt(view), lighting: LIGHTING });
    pool().bakeDemand(pass);
    const before = pass.frame({ ...inputAt(view), lighting: LIGHTING }).draws.at(-1)?.textures[
      "phaseFactorTable"
    ];
    // Six bright frosts of six L(0)s: rows past the table's first four.
    const frosts = edgeLayers();
    const photometry = new Float32Array(40);
    for (let k = 0; k < 6; k += 1) {
      photometry.set([0.9, 0.9, 0.9, 2, 0.9 - 0.05 * k], 5 * k);
    }
    pool().layers = { ...frosts, photometry };
    pool().deliver(pass, { face: 2, level: 3, i: 1, j: 1 });
    const after = pass.frame({ ...inputAt(view), lighting: LIGHTING }).draws.at(-1)?.textures[
      "phaseFactorTable"
    ];
    expect(before?.name).toBe("terrain phase table");
    expect(after?.name).toBe("terrain phase table");
    expect(after).not.toBe(before);
  });

  it("gives each drawn patch its parent's slot where the parent's horizon map is resident, and no parent otherwise", async () => {
    const { pass, pool, engine } = await passOn("low", { source: BODY });
    pool().layers = edgeLayers();
    const view = northPole(2_000_000);
    pass.frame({ ...inputAt(view), lighting: LIGHTING });
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      pool().deliver(pass, { face, level: 0, i: 0, j: 0 });
    }
    // Face 2's level-1 patches, three with their parent's map resident and one under a root whose
    // slot holds none.
    for (const [i, j] of [
      [0, 0],
      [1, 0],
      [0, 1],
      [1, 1],
    ] as const) {
      pool().deliver(pass, { face: 2, level: 1, i, j });
    }
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const parents = writtenParents(engine);
    expect(parents).toHaveLength(frame.drawSet.count);
    const roots = new Map(
      frame.drawSet.patches
        .filter(({ patch }) => patch.key.level === 0)
        .map(({ patch }) => [patch.key.face, patch.slot]),
    );
    const records = frame.drawSet.patches.map(({ patch }, n) => ({
      level: patch.key.level,
      face: patch.key.face,
      quadrant: (patch.key.i & 1) | ((patch.key.j & 1) << 1),
      written: parents[n] ?? [-1, -1],
    }));
    // A root has no parent; every other patch names its parent's slot and its quadrant there.
    expect(records.filter((r) => r.level === 0 && r.written[0] !== NO_PARENT_SLOT)).toEqual([]);
    const children = records.filter((r) => r.level > 0);
    expect(children.length).toBeGreaterThan(0);
    expect(
      children.filter((r) => r.written[0] === NO_PARENT_SLOT || r.written[1] !== r.quadrant),
    ).toEqual([]);
    expect(children.filter((r) => roots.has(r.face) && r.written[0] !== roots.get(r.face))).toEqual(
      [],
    );
    // A parent whose slot holds no map is no parent to blend to.
    pool().layers = {
      ...edgeLayers(),
      horizonNear: new Uint32Array(),
      horizonFar: new Uint32Array(),
    };
    pool().deliver(pass, { face: 2, level: 0, i: 0, j: 0 });
    const again = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const after = writtenParents(engine);
    const underBlank = again.drawSet.patches.flatMap(({ patch }, n) =>
      patch.key.face === 2 && patch.key.level === 1 ? [after[n]?.[0]] : [],
    );
    expect(underBlank.length).toBeGreaterThan(0);
    expect(underBlank.filter((slot) => slot !== NO_PARENT_SLOT)).toEqual([]);
  });

  it("gives R05's spike shading no parent's map, which it does not draw", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    pass.frame(inputAt(view));
    expect(writtenParents(engine).every(([slot]) => slot === NO_PARENT_SLOT)).toBe(true);
  });

  it("refuses a bake without R10's layers into a lit layout's slots", async () => {
    const { pass, pool } = await passOn("low", { source: BODY });
    const view = northPole(2_000_000);
    pass.frame({ ...inputAt(view), lighting: LIGHTING });
    expect(pool().bakeDemand(pass)).toBeGreaterThan(0);
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    expect(frame.drawSet.count).toBe(0);
  });
});

/** Each of a frame's draws' mesh, material, instance buffer and indirect buffer, by name. */
function named(frame: TerrainFrame): string[][] {
  return frame.draws.map((draw) => [
    draw.mesh.name,
    draw.material.name,
    draw.storageBuffers?.["instances"]?.name ?? "",
    draw.indirect?.buffer.name ?? "",
  ]);
}

describe("the interior patches (plan R14, R14.T1.a)", () => {
  const BODY = {
    kind: "body",
    body: "0200080020000000.0301",
    detailSeed: "617373656d626c79",
    weights: "baked",
  } as const;

  const LIGHTING = {
    lights: [],
    occluders: [],
    secondaries: [],
    elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
    exposureScale: 1e-4,
  } satisfies TerrainLighting;

  /** {@link LIGHTING} carrying the lava world's ground's emission about a star over the pole. */
  const EMITTING: TerrainLighting = {
    ...LIGHTING,
    emission: {
      field: surfaceTemperatureFieldOf(lavaWorldSurface()),
      star: { x: 0, y: 0, z: 1 },
      meltClasses: [],
    },
  };

  /**
   * A lit pass on low over `bodyLayers`, wholly surveyed, its patches baked, and its view, on an
   * engine that fails the materials `fails` names.
   */
  async function interiorPass(fails: (name: string) => boolean = () => false): Promise<{
    readonly pass: TerrainPass;
    readonly pool: FakePool;
    readonly engine: CountingRenderEngine;
    readonly view: TerrainView;
  }> {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    engine.failMaterials(fails);
    const pools: FakePool[] = [];
    const pass = new TerrainPass({
      engine,
      setting: "low",
      planet: PLANET,
      source: BODY,
      createPool: (bake) => {
        const made = new FakePool(bake);
        pools.push(made);
        return made;
      },
    });
    await pass.ready();
    const pool = pools.at(-1);
    if (pool === undefined) {
      throw new Error("no pool was made");
    }
    pool.layers = bodyLayers();
    const view = northPole(2_000_000);
    // Until nothing more is asked for, so that no patch is drawn by a stand-in.
    for (let round = 0; round < 6; round += 1) {
      pass.frame({ ...inputAt(view), lighting: LIGHTING });
      pool.bakeDemand(pass);
    }
    return { pass, pool, engine, view };
  }

  it("draws a wholly surveyed body's patches by the interior mesh and form alone", async () => {
    const { pass, engine, view } = await interiorPass();
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const path = pass.layout.vertexPath;
    expect(named(frame)).toEqual([
      [
        "terrain lit interior patch",
        terrainLitInteriorMaterialSpec(path).name,
        "terrain interior instances",
        "terrain interior indirect",
      ],
    ]);
    const slots = Array.from(frame.drawSet.slots.subarray(0, frame.drawSet.count));
    expect(slots.length).toBeGreaterThan(0);
    expect(writtenSlots(engine, "terrain interior instances")).toEqual(slots);
    const counts = engine.writes.filter((w) => w.buffer === "terrain interior indirect");
    expect([
      [...new Uint32Array(counts[0]?.data.slice().buffer ?? new ArrayBuffer(0))],
      [...new Uint32Array(counts.at(-1)?.data.slice().buffer ?? new ArrayBuffer(0))],
    ]).toEqual([[LIT_INTERIOR_INDICES, 0, 0, 0, 0], [slots.length]]);
    // The interior draw is the lit draw but its mesh, material, records and indirect arguments.
    const draw = frame.draws[0];
    expect([draw?.uniforms["bodyRotation"], draw?.textures["phaseFactorTable"]?.name]).toEqual([
      expect.any(Float32Array),
      "terrain phase table",
    ]);
  });

  it("draws the interior patches first and then the others, each in the draw set's order, and classifies a rebake again", async () => {
    const { pass, pool, engine, view } = await interiorPass();
    const first = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const drawn = first.drawSet.patches.map(({ patch }) => patch);
    expect(drawn.length).toBeGreaterThan(1);
    // Every other drawn patch baked again with a vertex outside the survey: an edge patch now.
    const edges = drawn.filter((_, n) => n % 2 === 1);
    for (const patch of edges) {
      pool.layersOf.set(patchKeyString(patch.key), edgeLayers());
      pool.deliver(pass, patch.key);
    }
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const path = pass.layout.vertexPath;
    expect(named(frame).map(([mesh, material]) => [mesh, material])).toEqual([
      ["terrain lit interior patch", terrainLitInteriorMaterialSpec(path).name],
      ["terrain lit patch", terrainLitMaterialSpec(path).name],
    ]);
    const edgeSlots = new Set(edges.map((patch) => patch.slot));
    const slots = Array.from(frame.drawSet.slots.subarray(0, frame.drawSet.count));
    expect([
      writtenSlots(engine, "terrain interior instances"),
      writtenSlots(engine, "terrain instances"),
    ]).toEqual([
      slots.filter((slot) => !edgeSlots.has(slot)),
      slots.filter((slot) => edgeSlots.has(slot)),
    ]);
  });

  it("counts the patches each draw draws", async () => {
    const { pass, pool, view } = await interiorPass();
    const first = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const edge = first.drawSet.patches[0]?.patch;
    if (edge === undefined) {
      throw new Error("the pass drew nothing");
    }
    pool.layersOf.set(patchKeyString(edge.key), edgeLayers());
    pool.deliver(pass, edge.key);
    const view0 = pass.drawFor(DEFAULT_TERRAIN_VIEW, { ...inputAt(view), lighting: LIGHTING });
    expect(view0.patches).toEqual({ interior: first.drawSet.count - 1, edge: 1 });
  });

  it("draws as before the split by the way back, from the same records, and splits again on the cluster path", async () => {
    const { pass, engine, view } = await interiorPass();
    const path = pass.layout.vertexPath;
    const split = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    pass.setGeometry(GEOMETRY_BEFORE_R14);
    const before = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const slots = Array.from(before.drawSet.slots.subarray(0, before.drawSet.count));
    expect([named(before), writtenSlots(engine), pass.geometry]).toEqual([
      [
        [
          "terrain lit patch",
          terrainLitMaterialSpec(path).name,
          "terrain instances",
          "terrain indirect",
        ],
      ],
      slots,
      GEOMETRY_BEFORE_R14,
    ]);
    pass.setGeometry(SETTINGS.low.geometry);
    const again = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    expect(named(again)).toEqual(named(split));
  });

  it("draws a stand-in by the lit mesh, never by the interior mesh, whatever its bake", async () => {
    const { pass, pool, engine } = await interiorPass();
    // Nearer, the selection asks for finer patches, which their resident ancestors stand in for.
    const nearer = northPole(200_000);
    pass.frame({ ...inputAt(nearer), lighting: LIGHTING });
    const frame = pass.frame({ ...inputAt(nearer), lighting: LIGHTING });
    const standIns = frame.drawSet.patches
      .filter(({ standIn }) => standIn)
      .map(({ patch }) => patch.slot);
    expect([standIns.length > 0, pool.demand.length > 0]).toEqual([true, true]);
    expect(standIns.filter((slot) => !writtenSlots(engine).includes(slot))).toEqual([]);
    expect(
      standIns.filter((slot) => writtenSlots(engine, "terrain interior instances").includes(slot)),
    ).toEqual([]);
  });

  it("draws every patch by the lit mesh where the interior form cannot be made, and reports it", async () => {
    const reported = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { pass, engine, view } = await interiorPass((name) =>
      name.startsWith("terrain-lit-interior"),
    );
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    expect(named(frame)).toEqual([
      [
        "terrain lit patch",
        terrainLitMaterialSpec(pass.layout.vertexPath).name,
        "terrain instances",
        "terrain indirect",
      ],
    ]);
    expect(writtenSlots(engine)).toEqual(
      Array.from(frame.drawSet.slots.subarray(0, frame.drawSet.count)),
    );
    expect(reported).toHaveBeenCalledWith(
      "the terrain's interior form failed: its interior patches are drawn with the others",
      expect.any(Error),
    );
  });

  it("draws the interior patches by the interior emissive form once both emissive forms are made (R08.T18.a)", async () => {
    const { pass, view } = await interiorPass();
    const path = pass.layout.vertexPath;
    const plain = pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    const emissive = pass.frame({ ...inputAt(view), lighting: EMITTING });
    expect([plain, emissive].map((frame) => frame.draws.map((draw) => draw.material.name))).toEqual(
      [
        [terrainLitInteriorMaterialSpec(path).name],
        [terrainLitEmissiveInteriorMaterialSpec(path).name],
      ],
    );
  });

  it("draws them by the lit mesh's emissive form where its interior form cannot be made", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { pass, view } = await interiorPass((name) =>
      name.startsWith("terrain-lit-emissive-interior"),
    );
    pass.frame({ ...inputAt(view), lighting: EMITTING });
    await microtasks();
    const frame = pass.frame({ ...inputAt(view), lighting: EMITTING });
    expect(named(frame).map(([mesh, material]) => [mesh, material])).toEqual([
      ["terrain lit patch", terrainLitEmissiveMaterialSpec(pass.layout.vertexPath).name],
    ]);
  });
});

describe("the terrain materials' storage bindings by stage (decision-r10-t10b-r09-t2e §5)", () => {
  const materials = (["baked-offsets", "face-differences"] as const).flatMap((path) => [
    terrainMaterialSpec(path),
    terrainLitMaterialSpec(path),
    terrainLitEmissiveMaterialSpec(path),
    terrainDepthMaterialSpec(path),
    terrainWireMaterialSpec(path),
  ]);

  it("count at most WebGPU's default eight storage buffers in each stage, by their declared stages", () => {
    const over = materials.filter((spec) => {
      const counts = storageBuffersPerStage(materialBindings(spec));
      return Math.max(counts.vertex, counts.fragment) > DEFAULT_STORAGE_BUFFERS_PER_STAGE;
    });
    expect(over.map((spec) => spec.name)).toEqual([]);
    // The lit material's: R05's five (four on face-differences) in the vertex stage, the class
    // layers, the lights and the horizon map in the fragment stage, and R10.T10.c's survey mask in
    // both.
    expect(
      storageBuffersPerStage(materialBindings(terrainLitMaterialSpec("baked-offsets"))),
    ).toEqual({ vertex: 6, fragment: 4 });
    expect(
      storageBuffersPerStage(materialBindings(terrainLitMaterialSpec("face-differences"))),
    ).toEqual({ vertex: 5, fragment: 4 });
    // The wireframe's (R10.T11): R05's five (four on face-differences), the survey mask and the
    // view's phases in the vertex stage, and the survey mask in the fragment stage.
    expect(
      storageBuffersPerStage(materialBindings(terrainWireMaterialSpec("baked-offsets"))),
    ).toEqual({ vertex: 7, fragment: 1 });
    expect(
      storageBuffersPerStage(materialBindings(terrainWireMaterialSpec("face-differences"))),
    ).toEqual({ vertex: 6, fragment: 1 });
  });

  it("declare each storage buffer to the stages whose entry points read it, and no other", () => {
    expect(materials.filter((spec) => spec.vertexWgsl !== spec.fragmentWgsl)).toEqual([]);
    const mismatched = materials.flatMap((spec) => {
      const read = bindingStages(spec.vertexWgsl);
      return (spec.storageBuffers ?? []).flatMap((buffer) => {
        const declared = [...(buffer.stages ?? ["vertex", "fragment"])].toSorted();
        const used = [...(read.get(buffer.binding) ?? [])].toSorted();
        return declared.join(" ") === used.join(" ") ? [] : [[spec.name, buffer.name, used]];
      });
    });
    expect(mismatched).toEqual([]);
    // The walk finds the reads at all: the lit material's horizon map in its fragment stage, and
    // its survey mask in both (R10.T10.c).
    const lit = bindingStages(terrainLitMaterialSpec("face-differences").vertexWgsl);
    expect([...(lit.get(16) ?? [])]).toEqual(["fragment"]);
    expect([...(lit.get(17) ?? [])].toSorted()).toEqual(["fragment", "vertex"]);
  });
});

/** `requests` by key string. */
function requestsByKey(requests: ReadonlyArray<PatchRequest>): Map<string, PatchRequest> {
  return new Map(requests.map((r) => [patchKeyString(r.key), r]));
}

describe("the terrain pass's lookahead (R10.T14.b)", () => {
  /** The low setting's pass on a clock the test sets, with each selection's input and result. */
  async function lookingAhead(): Promise<
    Awaited<ReturnType<typeof passOn>> & {
      readonly clock: { ms: number };
      readonly inputs: SelectionInput[];
      readonly selections: Selection[];
    }
  > {
    const clock = { ms: 0 };
    const inputs: SelectionInput[] = [];
    const selections: Selection[] = [];
    const made = await passOn("low", {
      lookaheadClockMs: () => clock.ms,
      onSelect: (input, selection) => {
        inputs.push(input);
        selections.push(selection);
      },
    });
    return { ...made, clock, inputs, selections };
  }

  /** {@link northPole}'s camera moved `xM` along +x: along the ground, at the pole. */
  function movedAlong(heightM: number, xM: number): TerrainView {
    const view = northPole(heightM);
    return { ...view, cameraM: { ...view.cameraM, x: xM } };
  }

  /**
   * Streams the pass in at 1.5 km over the pole on a still clock, then moves the camera 200 m along
   * the ground in 0.1 s, 2 km/s, which selects again: the lookahead's selection input there, with
   * the pass's own selection of the step.
   */
  async function movedFast(): Promise<
    Awaited<ReturnType<typeof lookingAhead>> & {
      readonly ahead: SelectionInput;
      readonly own: Selection;
    }
  > {
    const made = await lookingAhead();
    const { pass, pool, clock, inputs, selections } = made;
    streamed(pass, pool, northPole(1_500));
    clock.ms = 100;
    const frame = pass.frame(inputAt(movedAlong(1_500, 200), 16 * 100));
    const input = inputs.at(-1);
    const own = selections.at(-1);
    if (!frame.reselected || input === undefined || own === undefined) {
      throw new Error("a move of 200 m at 1.5 km selects again");
    }
    const before = northPole(1_500).cameraM;
    const after = movedAlong(1_500, 200).cameraM;
    // The pass's L before any bake took time to land: 0.25 s.
    const ahead = lookaheadInput(
      input,
      [{ velocityMps: scale(sub(after, before), 1 / 0.1), nearestM: 1_500 }],
      pass.lookaheadS,
    );
    if (ahead === null) {
      throw new Error("a camera at 2 km/s at 1.5 km selects ahead");
    }
    return { ...made, ahead, own };
  }

  it("asks for a moving camera's lookahead patches at half its weight", async () => {
    const { pass, pool, ahead, own } = await movedFast();
    expect(pass.lookaheadS).toBe(0.25);
    const asked = requestsByKey(pool().demand);
    const owned = requestsByKey(own.demand);
    const atWeightOne = requestsByKey(
      selectPatches({ ...ahead, views: ahead.views.map((view) => ({ ...view, weight: 1 })) })
        .demand,
    );
    const lookahead = selectPatches(ahead).demand;
    // Each at the larger of the two priorities; one only the lookahead asks for at half weight 1's.
    const expected = lookahead.map((request): readonly [string, number] => {
      const keyString = patchKeyString(request.key);
      const mine = owned.get(keyString);
      return [
        keyString,
        mine === undefined
          ? (atWeightOne.get(keyString)?.priority ?? NaN) * 0.5
          : Math.max(mine.priority, request.priority),
      ];
    });
    expect(expected.map(([keyString]) => asked.get(keyString)?.priority)).toEqual(
      expected.map(([, priority]) => priority),
    );
    expect(lookahead.filter((r) => !owned.has(patchKeyString(r.key))).length).toBeGreaterThan(0);
    // The selection's own demand is asked too, with every request's time to need.
    for (const keyString of owned.keys()) {
      expect(asked.has(keyString)).toBe(true);
    }
    expect(pool().demand.every((r) => r.needS !== undefined && r.needS >= 0)).toBe(true);
  });

  it("never draws the lookahead's patches, and pins them as demanded alone", async () => {
    const { pass, pool, clock, ahead, own } = await movedFast();
    const owned = requestsByKey(own.demand);
    const onlyAhead = selectPatches(ahead)
      .demand.map((r) => r.key)
      .filter((key) => !owned.has(patchKeyString(key)));
    expect(onlyAhead.length).toBeGreaterThan(0);
    for (const key of onlyAhead) {
      pool().deliver(pass, key);
    }
    // Until the next selection they are pinned as demanded: as many patches no view selects as
    // the cache has slots, shallower, which would go after them unpinned, evict none of them.
    const slots = pass.layout.slots.slotCount;
    for (let n = 0; n < slots; n += 1) {
      pool().deliver(pass, { face: 5, level: 8, i: n % 256, j: Math.floor(n / 256) });
    }
    expect(onlyAhead.filter((key) => pass.resident(patchKeyString(key)) === undefined)).toEqual([]);
    clock.ms = 200;
    // Still now: the arrivals select again, and the cache's pins are the selection's alone.
    const frame = pass.frame(inputAt(movedAlong(1_500, 200), 16 * 200));
    expect(frame.reselected).toBe(true);
    const chain = new Set<string>();
    for (const selected of frame.selection.patches.values()) {
      for (let key: PatchKey | null = selected.key; key !== null; key = parentKey(key)) {
        chain.add(patchKeyString(key));
      }
    }
    const drawn = new Set(
      frame.drawSet.patches.slice(0, frame.drawSet.count).map((p) => p.keyString),
    );
    for (const key of onlyAhead) {
      const keyString = patchKeyString(key);
      expect([chain.has(keyString), drawn.has(keyString)]).toEqual([false, false]);
    }
  });

  it("takes L from the median of its latest bakes' demand → resident times, within 0.25–2 s", async () => {
    const { pass, pool, clock } = await lookingAhead();
    const view = northPole(1_500);
    const askedAt = new Map<string, number>();
    const trips: number[] = [];
    const step = (ms: number): void => {
      clock.ms = ms;
      pass.frame(inputAt(view, ms));
      const asked = new Set(pool().demand.map((r) => patchKeyString(r.key)));
      for (const keyString of askedAt.keys()) {
        if (!asked.has(keyString)) {
          askedAt.delete(keyString);
        }
      }
      for (const keyString of asked) {
        if (!askedAt.has(keyString)) {
          askedAt.set(keyString, ms);
        }
      }
    };
    const deliver = (ms: number): void => {
      clock.ms = ms;
      for (const request of pool().demand) {
        const keyString = patchKeyString(request.key);
        trips.push((ms - (askedAt.get(keyString) ?? ms)) / 1000);
        askedAt.delete(keyString);
        pool().deliver(pass, request.key);
      }
    };
    const expected = (): number => lookaheadSeconds(trips.slice(-LOOKAHEAD_SAMPLES));
    expect(pass.lookaheadS).toBe(0.25);
    // The roots in 0.1 s: below the clamp.
    step(0);
    deliver(100);
    expect([pass.lookaheadS, expected()]).toEqual([0.25, 0.25]);
    // Their children in 1.2 s, which outnumber them.
    step(100);
    deliver(1_300);
    expect(pass.lookaheadS).toBe(expected());
    expect(pass.lookaheadS).toBeCloseTo(1.2, 12);
    // Then 8 s a bake, until they are most of the latest 64: above the clamp.
    for (let n = 0; trips.length < 2 * LOOKAHEAD_SAMPLES && n < 20; n += 1) {
      const ms = 1_300 + 8_000 * n;
      step(ms);
      deliver(ms + 8_000);
      expect(pass.lookaheadS).toBe(expected());
    }
    expect(pass.lookaheadS).toBe(2);
  });

  it("selects ahead at most every fourth selection while selection takes over 2 ms at p95", async () => {
    const { pass, pool, clock, selections } = await lookingAhead();
    streamed(pass, pool, northPole(1_500));
    // Each reading of the performance clock 3 ms after the last: every selection takes 3 ms.
    let nowMs = 0;
    const spy = vi.spyOn(performance, "now").mockImplementation(() => (nowMs += 3));
    const readings: number[] = [];
    for (let n = 1; n <= 12; n += 1) {
      clock.ms = 100 * n;
      const before = spy.mock.calls.length;
      const selected = selections.length;
      const frame = pass.frame(inputAt(movedAlong(1_500, 200 * n), 16 * n));
      expect([frame.reselected, selections.length]).toEqual([true, selected + 1]);
      readings.push(spy.mock.calls.length - before);
    }
    // Two readings for the selection's own, two more where the lookahead selects too.
    const aheadAt = readings.flatMap((count, n) => (count === 4 ? [n] : []));
    expect(readings.every((count) => count === 2 || count === 4)).toBe(true);
    expect(aheadAt).toEqual([0, 4, 8]);
    expect(LOOKAHEAD_SLOW_EVERY).toBe(4);
  });

  it("looks ahead of nothing without a clock, as the spike's frame selects", async () => {
    const { pass, pool } = await passOn("low");
    streamed(pass, pool, northPole(1_500));
    const frame = pass.frame(inputAt(movedAlong(1_500, 400), 16 * 100));
    expect(frame.reselected).toBe(true);
    expect(pool().demand).toEqual(
      frame.selection.demand.filter((r) => pass.resident(patchKeyString(r.key)) === undefined),
    );
  });
});

/** The half-float bits of `value`. */
function halfBits(value: number): number {
  return new Uint16Array(Float16Array.of(value).buffer)[0] ?? -1;
}

/** The normals tiles written, oldest first, each as its texels' half-float bits. */
function normalsTiles(engine: CountingRenderEngine): Uint16Array[] {
  return engine.textureWritten
    .filter((w) => w.texture === "terrain normals")
    .map((w) => new Uint16Array(w.data.buffer, w.data.byteOffset, w.data.byteLength / 2));
}

/** Sample (`x`, `y`)'s first component in a written tile of 129 × 129 and its gutter. */
function tileTexel(tile: Uint16Array, x: number, y: number): number {
  const stored = 129 + 2;
  return tile[2 * ((y + 1) * stored + (x + 1))] ?? -1;
}

describe("the terrain pass's drafted bakes (R10.T6.a's progressive bake)", () => {
  it("asks a draft's refinement at its request's priority and writes its normals alone, the vertices' kept", async () => {
    const { pass, pool, engine } = await passOn("high");
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    const fake = pool();
    const request = fake.demand[0];
    if (request === undefined) {
      throw new Error("nothing demanded");
    }
    fake.deliver(pass, request.key, { fill: 0.5, drafted: true });
    expect(fake.refines).toEqual([{ key: request.key, priority: request.priority, ticket: 1 }]);
    fake.refiningCount = 1;
    expect(pass.refining).toBe(1);
    const writes = engine.counts.bufferWrites;
    const tiles = normalsTiles(engine).length;
    fake.refined({
      key: request.key,
      ticket: 1,
      normals: new Float32Array(2 * NORMALS_BETWEEN).fill(0.25),
      complete: true,
    });
    const tile = normalsTiles(engine).at(-1);
    expect([normalsTiles(engine).length, engine.counts.bufferWrites]).toEqual([tiles + 1, writes]);
    if (tile === undefined) {
      throw new Error("no normals were written");
    }
    expect([0, 1, 2, 3].map((x) => tileTexel(tile, x, 0))).toEqual([
      halfBits(0.5),
      halfBits(0.25),
      halfBits(0.5),
      halfBits(0.25),
    ]);
    expect([tileTexel(tile, 0, 1), tileTexel(tile, 1, 1), tileTexel(tile, 128, 128)]).toEqual([
      halfBits(0.25),
      halfBits(0.25),
      halfBits(0.5),
    ]);
  });

  it("drops a draft's refinement once a whole bake or an eviction replaces it, and writes nothing for its ticket", async () => {
    const { pass, pool, engine } = await passOn("high");
    pass.frame(inputAt(northPole(2_000_000)));
    const fake = pool();
    const [first, second] = fake.demand;
    if (first === undefined || second === undefined) {
      throw new Error("two patches are demanded");
    }
    fake.deliver(pass, first.key, { fill: 0.5, drafted: true });
    fake.deliver(pass, first.key, { fill: 0.5, drafted: false });
    expect(fake.dropped).toEqual([patchKeyString(first.key)]);
    const tiles = normalsTiles(engine).length;
    fake.refined({
      key: first.key,
      ticket: 1,
      normals: new Float32Array(2 * NORMALS_BETWEEN),
      complete: true,
    });
    // A later draft's refinement is told with its own ticket alone.
    fake.deliver(pass, second.key, { fill: 0.5, drafted: true });
    fake.refined({
      key: second.key,
      ticket: 1,
      normals: new Float32Array(2 * NORMALS_BETWEEN),
      complete: true,
    });
    expect([normalsTiles(engine).length, fake.refines.map((r) => r.ticket)]).toEqual([
      tiles + 1,
      [1, 2],
    ]);
  });

  it.each([true, false])(
    "takes a bake as complete where its refinement is (%s), so that a block bakes it again where not",
    async (complete) => {
      const { pass, pool } = await passOn("high");
      pass.frame(inputAt(northPole(2_000_000)));
      const fake = pool();
      fake.layers = bodyLayers();
      const request = fake.demand[0];
      if (request === undefined) {
        throw new Error("nothing demanded");
      }
      fake.deliver(pass, request.key, { fill: 0.5, drafted: true });
      fake.refined({
        key: request.key,
        ticket: 1,
        normals: new Float32Array(2 * NORMALS_BETWEEN),
        complete,
      });
      let settled = false;
      pass.fieldChanged({ newlySurveyed: new Uint32Array(), newlyHeld: new Uint32Array() }, () => {
        settled = true;
      });
      expect(settled).toBe(complete);
    },
  );
});
