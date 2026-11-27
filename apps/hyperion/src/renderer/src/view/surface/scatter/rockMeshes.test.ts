import { describe, expect, it } from "vitest";

import { add, normalise, scale, vec3, type Vec3 } from "../../../geometry/vec3";
import { CountingRenderEngine, fakeDevice } from "../../../test/countingRenderEngine";
import { rockLayerFrom, rocksAbout } from "../../../test/rockFixtures";
import { InThreadRockPageBuilder, type RockPageBuilder } from "../../clusters/meshPageProducer";
import { ROOT_PAGE_BYTES } from "../../clusters/pageFormat";
import type { DrawSet, DrawnPatch } from "../../terrain/cache";
import type { GroundContact } from "../../terrain/grounded";
import { type PatchKey, patchKeyString } from "../../terrain/patchKey";
import type { BakedPatch, TerrainLayers } from "../../terrain/workers/messages";
import { ResidentRocks } from "./residentRocks";
import type { RockLayer } from "./rockLayer";
import { RockMeshes, type RockPoolRegion } from "./rockMeshes";
import { rocksToDraw } from "./rockSelection";

const RADIUS_M = 1_737_400;
/** Each test patch's bounding radius, m. */
const PATCH_RADIUS_M = 20;
/** The test patches' spacing along the east, m. */
const SPACING_M = 50;
/** A pool slot's records, LOD spheres and errors (`rockMeshes.ts`' own). */
const SLOT_RECORD_BYTES = 128 * (32 + 16 + 4);

const CENTRE = normalise(vec3(0.3, 1, 0.2));
const ORIGIN_M = scale(CENTRE, RADIUS_M);
const EAST = normalise(vec3(CENTRE.y, -CENTRE.x, 0));

/** Test patch `n`'s key, along one row of a face. */
function keyOf(n: number): PatchKey {
  return { face: 2, level: 17, i: 10 + n, j: 20 };
}

function keyStringOf(n: number): string {
  return patchKeyString(keyOf(n));
}

/** Each test patch's number by its key string. */
const NUMBERS = new Map(Array.from({ length: 10 }, (_, n) => [keyStringOf(n), n] as const));

/** Test patch `n`'s place: `n` spacings east of the first. */
function centreOf(n: number): Vec3 {
  return add(ORIGIN_M, scale(EAST, n * SPACING_M));
}

/** The same rock layer in every test patch, so that every mesh takes the same root slots. */
function layerOf(count = 6): RockLayer {
  return rockLayerFrom(rocksAbout(3, count, CENTRE, 12));
}

/** A finest patch's bake of `layer`, its rocks about the first patch's origin. */
function bakeOf(key: PatchKey, layer: RockLayer): BakedPatch {
  const layers: TerrainLayers = {
    palette: new Uint8Array(16),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(65 * 65),
    fieldLevel: 6,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
    photometry: new Float32Array(40),
    rocks: layer,
  };
  return {
    key,
    generation: 1,
    originM: ORIGIN_M,
    heights: new Float32Array(),
    offsets: null,
    normals: new Float16Array(),
    heightRangeM: [0, 0],
    boundingRadiusM: PATCH_RADIUS_M,
    originHeightM: 0,
    skirtDepthM: 0,
    layers,
  };
}

/** The region of `discs`, each test patch's sphere at its place. */
function regionOf(discs: ReadonlyArray<GroundContact>): RockPoolRegion {
  return {
    discs,
    sphereOf: (keyString) => {
      const n = NUMBERS.get(keyString);
      return n === undefined ? undefined : { centreM: centreOf(n), radiusM: PATCH_RADIUS_M };
    },
  };
}

/** A disc about test patch `n`'s place that meets the spheres of patches `n − 1` to `n + 1` alone. */
function discAt(n: number): GroundContact {
  return { positionM: centreOf(n), radiusM: SPACING_M - PATCH_RADIUS_M + 1 };
}

/** The in-thread builder's answers, a microtask each, and the pool's holds after them. */
async function landed(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

/** The root slots one test patch's mesh of `count` rocks takes. */
async function slotsOfOne(engine: CountingRenderEngine, count = 6): Promise<number> {
  const resident = new ResidentRocks();
  resident.take(keyStringOf(0), bakeOf(keyOf(0), layerOf(count)));
  const meshes = new RockMeshes(engine, new InThreadRockPageBuilder(), 64 * 1024 * 1024);
  meshes.sync(resident);
  await landed();
  const slots = meshes.pool.pressure().rootSlotsUsed;
  meshes.dispose();
  return slots;
}

/** A pool of `rootSlots` root slots, as `RockMeshes` sizes its pool from `pageBytes`. */
function meshesOf(
  engine: CountingRenderEngine,
  rootSlots: number,
  builder: RockPageBuilder = new InThreadRockPageBuilder(),
): RockMeshes {
  const meshes = new RockMeshes(
    engine,
    builder,
    2 * rootSlots * (ROOT_PAGE_BYTES + SLOT_RECORD_BYTES),
  );
  if (meshes.pool.layout.rootSlots !== rootSlots) {
    throw new Error("the test's slot bytes are no longer the pool's: see `rockMeshes.ts`");
  }
  return meshes;
}

/** The in-thread builder, failing while `failing` is set, counting the builds asked of it. */
class SwitchedBuilder implements RockPageBuilder {
  failing = false;
  asked = 0;
  readonly #inner = new InThreadRockPageBuilder();

  build(input: Parameters<RockPageBuilder["build"]>[0]): ReturnType<RockPageBuilder["build"]> {
    this.asked += 1;
    return this.failing ? Promise.resolve("the builder failed") : this.#inner.build(input);
  }

  dispose(): void {
    this.#inner.dispose();
  }
}

/** Test patches `order` resident, taken in that order, every one of the same rocks. */
function residentOf(order: ReadonlyArray<number>): ResidentRocks {
  const resident = new ResidentRocks();
  for (const n of order) {
    resident.take(keyStringOf(n), bakeOf(keyOf(n), layerOf()));
  }
  return resident;
}

/** The test patches whose meshes `meshes` holds, of the first `count`. */
function heldOf(meshes: RockMeshes, count: number): number[] {
  return Array.from({ length: count }, (_, n) => n).filter(
    (n) => meshes.get(keyStringOf(n)) !== undefined,
  );
}

/** A draw set of every test patch of the first `count`, each seen by view bit 0, drawn whole. */
function drawSetOf(count: number): Pick<DrawSet, "count" | "patches" | "views"> {
  const patches = Array.from({ length: count }, (_, n): DrawnPatch => ({
    keyString: keyStringOf(n),
    patch: {
      key: keyOf(n),
      generation: 1,
      originM: centreOf(n),
      heightRangeM: [0, 0],
      boundingRadiusM: PATCH_RADIUS_M,
      slot: n,
      keyString: keyStringOf(n),
    },
    standIn: false,
    within: null,
  }));
  return { count, patches, views: new Uint32Array(count).fill(1) };
}

describe("the rocks' pool holds a could-touch region's patches first (R11.T4.b, Design note 7)", () => {
  it("holds every region patch in a pool too small for every resident patch, evicting the farthest outside it", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    expect(s).toBeGreaterThan(0);
    // Room for three of six; the region (patches 0 and 1) baked last.
    const meshes = meshesOf(engine, 3 * s);
    meshes.sync(residentOf([2, 3, 4, 5, 0, 1]), regionOf([discAt(0)]));
    await landed();
    expect(heldOf(meshes, 6)).toEqual([0, 1, 2]);
    expect([3, 4, 5].map((n) => meshes.isRefused(keyStringOf(n)))).toEqual([true, true, true]);
    meshes.dispose();
  });

  it("takes the resident patches first come where it is given no region", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    meshes.sync(residentOf([2, 3, 4, 5, 0, 1]));
    await landed();
    expect(heldOf(meshes, 6)).toEqual([2, 3, 4]);
    meshes.dispose();
  });

  it("draws every region patch so, raising no detail limit for it", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    const region = regionOf([discAt(0)]);
    meshes.sync(residentOf([2, 3, 4, 5, 0, 1]), region);
    await landed();
    const drawn = rocksToDraw(drawSetOf(6), 1, 17, meshes, region.discs, "low");
    expect([drawn.regionRefused, drawn.patches.map((patch) => patch.keyString)]).toEqual([
      false,
      [keyStringOf(0), keyStringOf(1)],
    ]);
    meshes.dispose();
  });

  it("draws the same region patches whatever order their bakes land in: a function of the bodies, not the pool's history", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const region = regionOf([discAt(3)]);
    const drawSet = drawSetOf(6);
    const drawnBy = async (order: ReadonlyArray<number>): Promise<string[]> => {
      const meshes = meshesOf(engine, 3 * s);
      meshes.sync(residentOf(order), region);
      await landed();
      const { patches } = rocksToDraw(drawSet, 1, 17, meshes, region.discs, "high");
      meshes.dispose();
      return patches.filter((patch) => patch.rule === "region").map((patch) => patch.keyString);
    };
    const orders = [
      [0, 1, 2, 3, 4, 5],
      [5, 4, 3, 2, 1, 0],
      [0, 1, 2, 5, 3, 4],
    ];
    const seen = await Promise.all(orders.map(drawnBy));
    expect(seen).toEqual(orders.map(() => [2, 3, 4].map(keyStringOf)));
  });

  it("refuses a region patch where the region alone passes the pool, and says so", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    // Patches 0 to 3 the region's, four against room for three.
    const region = regionOf([discAt(0), discAt(2)]);
    meshes.sync(residentOf([0, 1, 2, 3, 4]), region);
    await landed();
    expect([heldOf(meshes, 5), meshes.isRefused(keyStringOf(3))]).toEqual([[0, 1, 2], true]);
    expect(rocksToDraw(drawSetOf(5), 1, 17, meshes, region.discs, "high").regionRefused).toBe(true);
    meshes.dispose();
  });

  it("asks no build again of a sync that leaves the room as it was", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const builder = new SwitchedBuilder();
    const meshes = meshesOf(engine, 3 * s, builder);
    const resident = residentOf([0, 1, 2, 3, 4]);
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    await landed();
    const asked = builder.asked;
    // A new step's region of the same discs.
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    expect([builder.asked, meshes.building]).toEqual([asked, 0]);
    meshes.dispose();
  });

  it("retries a refused region patch once a patch leaves, before a patch outside the region", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    const region = regionOf([discAt(0), discAt(2)]);
    const resident = residentOf([0, 1, 2, 3, 4]);
    meshes.sync(resident, region);
    await landed();
    resident.forget(keyStringOf(0));
    meshes.sync(resident, region);
    expect([meshes.isBuilding(keyStringOf(3)), meshes.isBuilding(keyStringOf(4))]).toEqual([
      true,
      false,
    ]);
    await landed();
    expect([heldOf(meshes, 5), meshes.isRefused(keyStringOf(3))]).toEqual([[1, 2, 3], false]);
    meshes.dispose();
  });

  it("evicts for a patch the region comes to hold, the farthest from the region first", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    const resident = residentOf([1, 2, 3, 4]);
    meshes.sync(resident);
    await landed();
    expect(heldOf(meshes, 5)).toEqual([1, 2, 3]);
    meshes.sync(resident, regionOf([{ positionM: centreOf(4), radiusM: 1 }]));
    expect(meshes.isBuilding(keyStringOf(4))).toBe(true);
    await landed();
    expect([heldOf(meshes, 5), meshes.isRefused(keyStringOf(1))]).toEqual([[2, 3, 4], true]);
    meshes.dispose();
  });

  it("retries a patch evicted for the region once room frees", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    const resident = residentOf([1, 2, 3, 4]);
    meshes.sync(resident);
    await landed();
    const region = regionOf([{ positionM: centreOf(4), radiusM: 1 }]);
    meshes.sync(resident, region);
    await landed();
    resident.forget(keyStringOf(2));
    meshes.sync(resident, region);
    expect(meshes.isBuilding(keyStringOf(1))).toBe(true);
    await landed();
    expect(heldOf(meshes, 5)).toEqual([1, 3, 4]);
    meshes.dispose();
  });

  it("retries a refused region patch once the region moves off meshes it may then evict", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    const resident = residentOf([0, 1, 2, 3]);
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    await landed();
    expect([heldOf(meshes, 4), meshes.isRefused(keyStringOf(3))]).toEqual([[0, 1, 2], true]);
    // The craft touches down by patch 3: the sweep's patches leave the region, resident still.
    meshes.sync(resident, regionOf([{ positionM: centreOf(3), radiusM: 1 }]));
    expect(meshes.isBuilding(keyStringOf(3))).toBe(true);
    await landed();
    expect([heldOf(meshes, 4), meshes.isRefused(keyStringOf(3))]).toEqual([[1, 2, 3], false]);
    meshes.dispose();
  });

  it("evicts nothing for a region mesh the meshes outside the region could not make room for", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const small = await slotsOfOne(engine, 1);
    expect(small).toBeLessThan(s);
    const meshes = meshesOf(engine, 2 * s + small);
    // The small patch outside the region first, then the region's three.
    const resident = new ResidentRocks();
    resident.take(keyStringOf(5), bakeOf(keyOf(5), layerOf(1)));
    for (const n of [0, 1, 2]) {
      resident.take(keyStringOf(n), bakeOf(keyOf(n), layerOf()));
    }
    meshes.sync(resident, regionOf([discAt(0), discAt(1)]));
    await landed();
    expect([heldOf(meshes, 6), meshes.isRefused(keyStringOf(2))]).toEqual([[0, 1, 5], true]);
    meshes.dispose();
  });

  it("refuses a mesh outside the region while a refused region patch waits on the room it would take", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const small = await slotsOfOne(engine, 1);
    // Room for two region meshes and the small one, but not a third region mesh.
    const meshes = meshesOf(engine, 2 * s + small);
    const resident = residentOf([0, 1, 2]);
    resident.take(keyStringOf(5), bakeOf(keyOf(5), layerOf(1)));
    meshes.sync(resident, regionOf([discAt(0), discAt(1)]));
    await landed();
    expect([heldOf(meshes, 6), meshes.isRefused(keyStringOf(5))]).toEqual([[0, 1], true]);
    meshes.dispose();
  });

  it("retries that mesh once the wait lifts", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const small = await slotsOfOne(engine, 1);
    const meshes = meshesOf(engine, 2 * s + small);
    const region = regionOf([discAt(0), discAt(1)]);
    const resident = residentOf([0, 1, 2]);
    resident.take(keyStringOf(5), bakeOf(keyOf(5), layerOf(1)));
    meshes.sync(resident, region);
    await landed();
    // The waiting region patch leaves, and with it the wait.
    resident.forget(keyStringOf(2));
    meshes.sync(resident, region);
    expect(meshes.isBuilding(keyStringOf(5))).toBe(true);
    await landed();
    expect(heldOf(meshes, 6)).toEqual([0, 1, 5]);
    meshes.dispose();
  });

  it("asks a retry whose build fails no more until its room grows", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const builder = new SwitchedBuilder();
    const meshes = meshesOf(engine, 3 * s, builder);
    const resident = residentOf([0, 1, 2, 3]);
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    await landed();
    builder.failing = true;
    resident.forget(keyStringOf(0));
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    await landed();
    const asked = builder.asked;
    for (let step = 0; step < 3; step += 1) {
      meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    }
    expect([builder.asked, heldOf(meshes, 4)]).toEqual([asked, [1, 2]]);
    builder.failing = false;
    resident.forget(keyStringOf(1));
    meshes.sync(resident, regionOf([discAt(0), discAt(2)]));
    await landed();
    expect(heldOf(meshes, 4)).toEqual([2, 3]);
    meshes.dispose();
  });

  it("shares one freeing's room among its retries, asking no more than it holds", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    const resident = residentOf([0, 1, 2, 3, 4, 5]);
    meshes.sync(resident);
    await landed();
    resident.forget(keyStringOf(0));
    meshes.sync(resident);
    expect([3, 4, 5].map((n) => meshes.isBuilding(keyStringOf(n)))).toEqual([true, false, false]);
    await landed();
    expect(heldOf(meshes, 6)).toEqual([1, 2, 3]);
    meshes.dispose();
  });
});
