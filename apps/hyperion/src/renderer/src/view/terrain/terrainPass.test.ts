import { afterEach, describe, expect, it } from "vitest";

import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import { bindingStages } from "../../test/wgslStages";
import { materialBindings, storageBuffersPerStage } from "../engine/webgpu/materials";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION, rotation3FromRows } from "../coords/rotation";
import type { Vec3 } from "../../geometry/vec3";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { TERRAIN_SETTINGS, terrainSettingsFor } from "../quality/qualitySetting";
import { terrainMaterialSpec } from "./gpu/material";
import { normalsAtlasOf } from "./gpu/resources";
import { CLASS_LAWS_BYTES, terrainSlotLayout } from "./slotLayout";
import { terrainWireMaterialSpec } from "./terrainWire";
import { distanceToBoxM, relativeBounds } from "./bounds";
import type { DrawSet } from "./cache";
import { vertexDir } from "./cube";
import { finestPatchSizeM } from "./grounded";
import { INSTANCE_RECORD_BYTES, NO_PARENT_SLOT } from "./gpu/uniforms";
import { type Edge, EDGES, edgeNeighbour, parentKey, patchKeyString } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import {
  type PatchRequest,
  screenSpaceErrorPx,
  type Selection,
  type SelectionInput,
  selectionErrorM,
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
  terrainLitMaterialSpec,
} from "./terrainMaterial";
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
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

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
  terminated = false;

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

  deliver(pass: TerrainPass, key: PatchRequest["key"]): void {
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
    Pick<TerrainPassOptions, "measureSelection" | "onResident" | "onSelect" | "terrain" | "source">
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

/** The slots of the instance records last written. */
function writtenSlots(engine: CountingRenderEngine): number[] {
  const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
  const u = new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
  const slots: number[] = [];
  for (let i = 0; i < u.length / (INSTANCE_RECORD_BYTES / 4); i += 1) {
    slots.push(u[i * 8 + 3] ?? -1);
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
    expect(frame.draw?.indirect?.buffer.name).toBe("terrain indirect");
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
      expect([f[i * 8 + 4], f[i * 8 + 5]]).toEqual(levelOne.map((m) => Math.fround(m)));
    }
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
    expect(frame.draw?.material.name).toBe(terrainMaterialSpec("face-differences").name);
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
    expect(Array.from(frame.draw?.uniforms["bodyRotation"] ?? []).slice(0, 4)).toEqual([
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
    bands.set(drawn.keyString, [f[i * 8 + 4] ?? Number.NaN, f[i * 8 + 5] ?? Number.NaN]);
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
 * The parent slots and quadrants of the instance records last written, each quadrant without the
 * morph holds above its two bits (R10.T8.c).
 */
function writtenParents(engine: CountingRenderEngine): Array<readonly [number, number]> {
  const write = engine.writes.findLast((w) => w.buffer === "terrain instances");
  const u = new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(0));
  const parents: Array<readonly [number, number]> = [];
  for (let i = 0; i < u.length / (INSTANCE_RECORD_BYTES / 4); i += 1) {
    const quadrant = u[i * 8 + 7];
    parents.push([u[i * 8 + 6] ?? -1, quadrant === undefined ? -1 : quadrant & 3]);
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
    pool().layers = bodyLayers();
    const view = northPole(2_000_000);
    pass.frame({ ...inputAt(view), lighting: LIGHTING });
    pool().bakeDemand(pass);
    const frame = pass.frame({ ...inputAt(view), lighting: LIGHTING });
    const draw = frame.draw;
    if (draw === null) {
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
    pool().layers = bodyLayers();
    const view = northPole(2_000_000);
    pass.frame(inputAt(view));
    pool().bakeDemand(pass);
    const unlit = pass.frame(inputAt(view));
    expect(unlit.draw).not.toBeNull();
    expect(engine.writes.filter((w) => w.buffer === "terrain lights")).toHaveLength(1);
    const air = createAirlessTables(engine, "a body's air");
    const withAir = pass.frame({ ...inputAt(view), lighting: { ...LIGHTING, atmosphere: air } });
    expect(withAir.draw?.textures["atmosphereBody"]).toBe(air.body);
  });

  it("binds the larger phase table once the slots' laws outgrow it", async () => {
    const { pass, pool } = await passOn("low", { source: BODY });
    pool().layers = bodyLayers();
    const view = northPole(2_000_000);
    pass.frame({ ...inputAt(view), lighting: LIGHTING });
    pool().bakeDemand(pass);
    const before = pass.frame({ ...inputAt(view), lighting: LIGHTING }).draw?.textures[
      "phaseFactorTable"
    ];
    // Six bright frosts of six L(0)s: rows past the table's first four.
    const frosts = bodyLayers();
    const photometry = new Float32Array(40);
    for (let k = 0; k < 6; k += 1) {
      photometry.set([0.9, 0.9, 0.9, 2, 0.9 - 0.05 * k], 5 * k);
    }
    pool().layers = { ...frosts, photometry };
    pool().deliver(pass, { face: 2, level: 3, i: 1, j: 1 });
    const after = pass.frame({ ...inputAt(view), lighting: LIGHTING }).draw?.textures[
      "phaseFactorTable"
    ];
    expect(before?.name).toBe("terrain phase table");
    expect(after?.name).toBe("terrain phase table");
    expect(after).not.toBe(before);
  });

  it("gives each drawn patch its parent's slot where the parent's horizon map is resident, and no parent otherwise", async () => {
    const { pass, pool, engine } = await passOn("low", { source: BODY });
    pool().layers = bodyLayers();
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
      ...bodyLayers(),
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

describe("the terrain materials' storage bindings by stage (decision-r10-t10b-r09-t2e §5)", () => {
  const materials = (["baked-offsets", "face-differences"] as const).flatMap((path) => [
    terrainMaterialSpec(path),
    terrainLitMaterialSpec(path),
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
