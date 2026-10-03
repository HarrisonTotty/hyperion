import { describe, expect, it } from "vitest";

import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION, rotation3FromRows } from "../coords/rotation";
import type { Vec3 } from "../../geometry/vec3";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";
import { INSTANCE_RECORD_BYTES } from "./gpu/uniforms";
import { patchKeyString } from "./patchKey";
import { planetGeometry } from "./planet";
import type { PatchRequest } from "./select";
import {
  morphRangeM,
  RESELECT_FRACTION,
  type TerrainFrameInput,
  TerrainPass,
  type TerrainPool,
  type TerrainView,
} from "./terrainPass";
import type { BakedPatch, BakeSettings } from "./workers/messages";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** A pool that records the demand and bakes on the test's command. */
class FakePool implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  readonly #listeners = new Set<(bake: BakedPatch) => void>();
  terminated = false;

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
): Promise<{ pass: TerrainPass; pool: () => FakePool; engine: CountingRenderEngine }> {
  const counting = await countingRenderEngine({
    maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
    maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  });
  const pools: FakePool[] = [];
  const pass = new TerrainPass({
    engine: counting,
    setting,
    planet: PLANET,
    ridges: "off",
    createPool: (bake) => {
      const pool = new FakePool(bake);
      pools.push(pool);
      return pool;
    },
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

  it("budgets half the high setting's BakedOffsets slots for selection", async () => {
    expect((await passOn("high")).pass.maxPatches).toBe(981);
  });

  it("budgets half the low setting's slots for selection", async () => {
    expect((await passOn("low")).pass.maxPatches).toBe(648);
  });

  it("selects no more patches than its budget, and says the budget bound", async () => {
    const { pass, pool } = await passOn("high");
    // 1.5 km up, 69° from straight down, at 1080p: even under the fake pool's flat bakes, whose
    // ranges are tight, the high setting wants several thousand patches, beyond its budget of 981.
    // Selection descends as bakes land (R05.T7's streaming gate), so the demand is baked until it
    // empties.
    const view = { ...tilted(1_500, 1.2), viewport: { widthPx: 1920, heightPx: 1080 } };
    let frame = pass.frame(inputAt(view));
    expect(frame.selection.patches.size).toBeLessThanOrEqual(pass.maxPatches);
    for (let n = 1; n < 60 && pool().bakeDemand(pass) > 0; n += 1) {
      frame = pass.frame(inputAt(view, 16 * n));
      expect(frame.selection.patches.size).toBeLessThanOrEqual(pass.maxPatches);
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
    expect(pass.frame(inputAt(northPole(heightM * (1 - 2 * RESELECT_FRACTION)))).reselected).toBe(
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
