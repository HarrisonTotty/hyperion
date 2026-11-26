import { describe, expect, it } from "vitest";

import { add, normalise, scale, vec3, type Vec3 } from "../../../geometry/vec3";
import { CountingRenderEngine, fakeDevice } from "../../../test/countingRenderEngine";
import { rockLayerFrom, rocksAbout } from "../../../test/rockFixtures";
import { InThreadRockPageBuilder } from "../../clusters/meshPageProducer";
import { ROOT_PAGE_BYTES } from "../../clusters/pageFormat";
import type { DrawSet, DrawnPatch } from "../../terrain/cache";
import type { GroundContact } from "../../terrain/grounded";
import type { PatchKey } from "../../terrain/patchKey";
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
  return `2/17/${String(10 + n)}/20`;
}

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
      const n = Number(keyString.split("/")[2]) - 10;
      return { centreM: centreOf(n), radiusM: PATCH_RADIUS_M };
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
function meshesOf(engine: CountingRenderEngine, rootSlots: number): RockMeshes {
  return new RockMeshes(
    engine,
    new InThreadRockPageBuilder(),
    2 * rootSlots * (ROOT_PAGE_BYTES + SLOT_RECORD_BYTES),
  );
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
    const region = regionOf([discAt(0)]);
    const resident = residentOf([2, 3, 4, 5, 0, 1]);
    meshes.sync(resident, region);
    await landed();
    expect(heldOf(meshes, 6)).toEqual([0, 1, 2]);
    expect([0, 1, 2, 3, 4, 5].map((n) => meshes.isRefused(keyStringOf(n)))).toEqual([
      false,
      false,
      false,
      true,
      true,
      true,
    ]);
    // Without the region, first come.
    const firstCome = meshesOf(engine, 3 * s);
    firstCome.sync(residentOf([2, 3, 4, 5, 0, 1]));
    await landed();
    expect(heldOf(firstCome, 6)).toEqual([2, 3, 4]);
    // So the region raises no detail limit, which first come would.
    const drawSet = drawSetOf(6);
    const drawn = rocksToDraw(drawSet, 1, 17, meshes, region.discs, "low");
    expect([drawn.regionRefused, drawn.patches.map((patch) => patch.keyString)]).toEqual([
      false,
      [keyStringOf(0), keyStringOf(1)],
    ]);
    expect(rocksToDraw(drawSet, 1, 17, firstCome, region.discs, "low").regionRefused).toBe(true);
    meshes.dispose();
    firstCome.dispose();
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
      const { patches, regionRefused } = rocksToDraw(drawSet, 1, 17, meshes, region.discs, "high");
      meshes.dispose();
      expect(regionRefused).toBe(false);
      return patches.filter((patch) => patch.rule === "region").map((patch) => patch.keyString);
    };
    const orders = [
      [0, 1, 2, 3, 4, 5],
      [5, 4, 3, 2, 1, 0],
      [0, 1, 2, 5, 3, 4],
    ];
    const seen = await Promise.all(orders.map(drawnBy));
    expect(seen).toEqual(orders.map(() => [keyStringOf(2), keyStringOf(3), keyStringOf(4)]));
  });

  it("refuses the region only where it alone passes the pool, and retries a refused patch once room frees", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const meshes = meshesOf(engine, 3 * s);
    // Patches 0 to 3 the region's, four against room for three.
    const region = regionOf([discAt(0), discAt(2)]);
    const resident = residentOf([0, 1, 2, 3, 4]);
    meshes.sync(resident, region);
    await landed();
    expect([heldOf(meshes, 5), meshes.isRefused(keyStringOf(3))]).toEqual([[0, 1, 2], true]);
    const drawSet = drawSetOf(5);
    expect(rocksToDraw(drawSet, 1, 17, meshes, region.discs, "high").regionRefused).toBe(true);
    // A sync with nothing freed asks nothing again.
    meshes.sync(resident, region);
    expect(meshes.building).toBe(0);
    // Patch 0 leaves: its room goes to the region's refused patch, not to patch 4 outside it.
    resident.forget(keyStringOf(0));
    meshes.sync(resident, region);
    expect([meshes.isBuilding(keyStringOf(3)), meshes.isBuilding(keyStringOf(4))]).toEqual([
      true,
      false,
    ]);
    await landed();
    expect([heldOf(meshes, 5), meshes.isRefused(keyStringOf(3))]).toEqual([[1, 2, 3], false]);
    expect(rocksToDraw(drawSet, 1, 17, meshes, region.discs, "high").regionRefused).toBe(false);
    // The bodies move on: the region holds patch 4 alone, which evicts the farthest from it.
    const moved = regionOf([{ positionM: centreOf(4), radiusM: 1 }]);
    meshes.sync(resident, moved);
    expect(meshes.isBuilding(keyStringOf(4))).toBe(true);
    await landed();
    expect(heldOf(meshes, 5)).toEqual([2, 3, 4]);
    // Patch 2 leaves, and patch 1, evicted, outside the region now, is retried into its room.
    resident.forget(keyStringOf(2));
    meshes.sync(resident, moved);
    expect(meshes.isBuilding(keyStringOf(1))).toBe(true);
    await landed();
    expect(heldOf(meshes, 5)).toEqual([1, 3, 4]);
    meshes.dispose();
  });

  it("refuses a mesh outside the region while a refused region patch waits on the room it would take", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const s = await slotsOfOne(engine);
    const small = await slotsOfOne(engine, 1);
    expect(small).toBeLessThan(s);
    // Room for two region meshes and the small one, but not a third region mesh.
    const meshes = meshesOf(engine, 2 * s + small);
    const region = regionOf([discAt(0), discAt(1)]);
    const resident = residentOf([0, 1, 2]);
    resident.take(keyStringOf(5), bakeOf(keyOf(5), layerOf(1)));
    meshes.sync(resident, region);
    await landed();
    expect([heldOf(meshes, 6), meshes.isRefused(keyStringOf(2))]).toEqual([[0, 1], true]);
    expect(meshes.isRefused(keyStringOf(5))).toBe(true);
    meshes.dispose();
  });
});
