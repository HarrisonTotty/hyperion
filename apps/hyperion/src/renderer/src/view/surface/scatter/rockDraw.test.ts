import { describe, expect, it, vi } from "vitest";

import { normalise, vec3 } from "../../../geometry/vec3";
import { CountingRenderEngine, fakeDevice } from "../../../test/countingRenderEngine";
import { rockLayerFrom, rocksAbout } from "../../../test/rockFixtures";
import { CLUSTER_BASE_UNIFORM } from "../../clusters/buckets";
import { CLUSTER_CULL_KERNEL } from "../../clusters/cull";
import { InThreadRockPageBuilder } from "../../clusters/meshPageProducer";
import { pageSlotOffsetBytes } from "../../clusters/pages";
import { RESOLVE_SCREEN_MESH } from "../../clusters/resolve";
import type { WgslMaterialSpec } from "../../engine/types";
import type { PatchKey } from "../../terrain/patchKey";
import type { BakedPatch, TerrainLayers } from "../../terrain/workers/messages";
import { ResidentRocks } from "./residentRocks";
import {
  ROCK_CLUSTER_MESH,
  ROCK_CULL_LABEL,
  ROCK_INSTANCE_BYTES,
  ROCK_PIXEL_PASS,
  ROCK_REGION_PASS,
  RockView,
} from "./rockDraw";
import type { RockLayer } from "./rockLayer";
import { RockMeshes } from "./rockMeshes";
import { ROCK_RECORD_WORDS } from "./rockPages";

const RADIUS_M = 1_737_400;

/** A finest patch's bake of `layer` about `originM`. */
function bakeOf(key: PatchKey, originM: ReturnType<typeof vec3>, layer: RockLayer): BakedPatch {
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
    originM,
    heights: new Float32Array(),
    offsets: null,
    normals: new Float16Array(),
    heightRangeM: [0, 0],
    boundingRadiusM: 20,
    originHeightM: 0,
    skirtDepthM: 0,
    layers,
  };
}

/** Two patches' rocks held, built and in the pool. */
async function heldRocks(engine: CountingRenderEngine): Promise<{
  readonly meshes: RockMeshes;
  readonly keys: readonly [string, string];
  readonly originM: ReturnType<typeof vec3>;
}> {
  const centre = normalise(vec3(0.3, 1, 0.2));
  const originM = vec3(centre.x * RADIUS_M, centre.y * RADIUS_M, centre.z * RADIUS_M);
  const resident = new ResidentRocks();
  const a: PatchKey = { face: 2, level: 17, i: 10, j: 20 };
  const b: PatchKey = { face: 2, level: 17, i: 11, j: 20 };
  resident.take("2/17/10/20", bakeOf(a, originM, rockLayerFrom(rocksAbout(3, 6, centre, 12))));
  resident.take("2/17/11/20", bakeOf(b, originM, rockLayerFrom(rocksAbout(5, 4, centre, 12))));
  const meshes = new RockMeshes(engine, new InThreadRockPageBuilder(), 32 * 1024 * 1024);
  meshes.sync(resident);
  expect(meshes.building).toBe(2);
  // The in-thread builder answers a microtask later.
  await Promise.resolve();
  await Promise.resolve();
  expect([meshes.size, meshes.building, meshes.refused]).toEqual([2, 0, 0]);
  return { meshes, keys: ["2/17/10/20", "2/17/11/20"], originM };
}

/** A material of no shader, named `name`, for the fake engine. */
function emptySpec(name: string): WgslMaterialSpec {
  return {
    name,
    displayName: name.toUpperCase(),
    vertexWgsl: "",
    fragmentWgsl: "",
    uniforms: [],
    samplers: [],
    textures: [],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
  };
}

describe("a view's rock draw (R11.T4.b, R14.T5.c)", () => {
  it("culls each patch's pages once into its rule's pass, its instances placed about the camera", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const { meshes, keys, originM } = await heldRocks(engine);
    const view = new RockView(engine, "primary");
    const camera = vec3(originM.x + 3, originM.y - 250, originM.z + 4);
    const material = engine.createMaterial({
      name: "rock test",
      displayName: "ROCK TEST",
      vertexWgsl: "",
      fragmentWgsl: "",
      uniforms: [{ name: "clusterBase", type: "u32" }],
      samplers: [],
      textures: [],
      cullMode: "back",
      depthWrite: true,
      colourWrites: true,
      blend: "none",
    });
    const { forward: draws, byIds } = view.frame({
      patches: [
        { keyString: keys[0], terrainSlot: 7, rule: "pixel" },
        { keyString: keys[1], terrainSlot: 9, rule: "region" },
      ],
      meshes,
      cameraBodyFixedM: camera,
      rotationRows: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
      planes: [],
      horizon: null,
      toleranceRad: 1e-3,
      viewAxis: vec3(0, 0, -1),
      kernel: engine.createCompute(CLUSTER_CULL_KERNEL),
      mesh: engine.createMesh(ROCK_CLUSTER_MESH.mesh),
      base: { material, offsetFromCameraM: new Float32Array(3), uniforms: {}, textures: {} },
      ids: null,
    });
    expect(byIds).toBeNull();
    const pages = keys.map((key) => {
      const held = meshes.get(key);
      return held === undefined ? [] : meshes.pool.residentPages(held.mesh);
    });
    const count = (pages[0]?.length ?? 0) + (pages[1]?.length ?? 0);
    const dispatch = engine.dispatched.at(-1);
    expect([dispatch?.kernel, dispatch?.pass]).toEqual(["cluster cull", ROCK_CULL_LABEL]);
    expect(view.counts).toMatchObject({ patches: 2, instances: count, rocks: 10 });
    const instances = engine.writes.find((w) => w.buffer === "primary rock instances");
    const inputs = engine.writes.findLast((w) => w.buffer === "rocks cull inputs");
    if (instances === undefined || inputs === undefined) {
      throw new Error("the frame writes its instances and its cull inputs");
    }
    const words = new DataView(instances.data.buffer, instances.data.byteOffset);
    const bits = new Uint32Array(inputs.data.buffer, inputs.data.byteOffset, 2 * count);
    let n = 0;
    keys.forEach((key, p) => {
      const held = meshes.get(key);
      const pageOrigin = held === undefined ? undefined : meshes.pool.originOf(held.mesh);
      if (held === undefined || pageOrigin === undefined) {
        throw new Error("both patches are held");
      }
      for (const [, slot] of pages[p] ?? []) {
        const at = n * ROCK_INSTANCE_BYTES;
        expect([0, 4, 8].map((k) => words.getFloat32(at + k, true))).toEqual(
          [
            originM.x + pageOrigin.x - camera.x,
            originM.y + pageOrigin.y - camera.y,
            originM.z + pageOrigin.z - camera.z,
          ].map(Math.fround),
        );
        expect([12, 16, 20, 24].map((k) => words.getUint32(at + k, true))).toEqual([
          slot,
          pageSlotOffsetBytes(meshes.pool.layout, slot) / 4,
          p === 0 ? 7 : 9,
          p === 0 ? 0 : 6,
        ]);
        expect(bits[2 * n]).toBe(1 << (p === 0 ? ROCK_PIXEL_PASS : ROCK_REGION_PASS));
        n += 1;
      }
    });
    const records = engine.writes.find((w) => w.buffer === "primary rock records");
    expect(records?.bytes).toBe(10 * ROCK_RECORD_WORDS * 4);
    expect(draws).toHaveLength(2);
    for (const draw of draws) {
      expect(Object.keys(draw.storageBuffers ?? {}).toSorted()).toEqual([
        "clusterLists",
        "rockInstances",
        "rockPages",
        "rockRecords",
      ]);
      expect(draw.uniforms[CLUSTER_BASE_UNIFORM]).toBeDefined();
    }
    view.dispose();
    meshes.dispose();
  });

  it("draws nothing of a patch whose rocks are not built, nor of none", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const { meshes } = await heldRocks(engine);
    const view = new RockView(engine, "primary");
    const before = engine.dispatched.length;
    const frame = {
      patches: [{ keyString: "2/17/99/99", terrainSlot: 0, rule: "pixel" as const }],
      meshes,
      cameraBodyFixedM: vec3(0, 0, 0),
      rotationRows: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)] as const,
      planes: [],
      horizon: null,
      toleranceRad: 1e-3,
      viewAxis: vec3(0, 0, -1),
      kernel: engine.createCompute(CLUSTER_CULL_KERNEL),
      mesh: engine.createMesh(ROCK_CLUSTER_MESH.mesh),
      ids: null,
      base: {
        material: engine.createMaterial({
          name: "rock test",
          displayName: "ROCK TEST",
          vertexWgsl: "",
          fragmentWgsl: "",
          uniforms: [],
          samplers: [],
          textures: [],
          cullMode: "back",
          depthWrite: true,
          colourWrites: true,
          blend: "none",
        }),
        offsetFromCameraM: new Float32Array(3),
        uniforms: {},
        textures: {},
      },
    };
    expect(view.frame(frame)).toEqual({ forward: [], byIds: null });
    expect(view.frame({ ...frame, patches: [] })).toEqual({ forward: [], byIds: null });
    expect(engine.dispatched.length).toBe(before);
    view.dispose();
    meshes.dispose();
  });

  it("draws by the visibility buffer its buckets' ID raster and one resolve over its whole lists, nothing forward (R14.T5.e)", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const { meshes, keys, originM } = await heldRocks(engine);
    const view = new RockView(engine, "primary");
    const forward = engine.createMaterial(emptySpec("rock forward"));
    const raster = engine.createMaterial(emptySpec("rock id"));
    const resolveMaterial = engine.createMaterial(emptySpec("rock resolve"));
    const screen = engine.createMesh(RESOLVE_SCREEN_MESH);
    const base = {
      material: forward,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    const input = {
      patches: [
        { keyString: keys[0], terrainSlot: 7, rule: "pixel" as const },
        { keyString: keys[1], terrainSlot: 9, rule: "region" as const },
      ],
      meshes,
      cameraBodyFixedM: vec3(originM.x + 3, originM.y - 250, originM.z + 4),
      rotationRows: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)] as const,
      planes: [],
      horizon: null,
      toleranceRad: 1e-3,
      viewAxis: vec3(0, 0, -1),
      kernel: engine.createCompute(CLUSTER_CULL_KERNEL),
      mesh: engine.createMesh(ROCK_CLUSTER_MESH.mesh),
      base,
      ids: {
        idBase: { ...base, material: raster },
        resolve: { ...base, mesh: screen, material: resolveMaterial },
        subpixelBits: 8,
      },
    };
    const { forward: drawn, byIds } = view.frame(input);
    if (byIds === null) {
      throw new Error("a frame by IDs draws its rocks by them");
    }
    const [resolve] = byIds.resolve;
    expect([drawn, byIds.raster.length, byIds.resolve.length]).toEqual([[], 2, 1]);
    expect(byIds.raster.every((draw) => draw.material === raster)).toBe(true);
    expect([resolve?.material, resolve?.mesh]).toEqual([resolveMaterial, screen]);
    const lists = byIds.raster[0]?.storageBuffers?.["clusterLists"];
    expect(resolve?.storageBuffers?.["clusterLists"]).toBe(lists);
    expect(Object.keys(resolve?.storageBuffers ?? {}).toSorted()).toEqual([
      "clusterLists",
      "rockInstances",
      "rockPages",
      "rockRecords",
    ]);
    // Its range: the rocks' whole lists, both passes', from 0 until the frame moves it.
    const entries = Array.from(resolve?.uniforms["resolveLists"] ?? []);
    expect([entries[1], entries[2], entries[3]]).toEqual([0, byIds.entries, 256]);
    expect(byIds.entries).toBe(2 * (entries[0] ?? 0));
    // The same draws the next frame, while nothing they hold changes.
    const next = view.frame(input).byIds;
    expect([next?.raster[0] === byIds.raster[0], next?.resolve[0] === resolve]).toEqual([
      true,
      true,
    ]);
    view.dispose();
    meshes.dispose();
  });

  it("refuses a mesh its pool cannot hold whole, its patch named refused until the patch leaves", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const centre = normalise(vec3(0.3, 1, 0.2));
    const originM = vec3(centre.x * RADIUS_M, centre.y * RADIUS_M, centre.z * RADIUS_M);
    const resident = new ResidentRocks();
    const key: PatchKey = { face: 2, level: 17, i: 10, j: 20 };
    resident.take("2/17/10/20", bakeOf(key, originM, rockLayerFrom(rocksAbout(3, 40, centre, 12))));
    // A pool of two root slots, against the many pages of forty rocks.
    const meshes = new RockMeshes(
      engine,
      new InThreadRockPageBuilder(),
      2 * 2 * (16 * 1024 + 6_656),
    );
    meshes.sync(resident);
    await Promise.resolve();
    await Promise.resolve();
    expect([meshes.size, meshes.refused, meshes.isRefused("2/17/10/20")]).toEqual([0, 1, true]);
    expect(meshes.pool.pressure().rootSlotsUsed).toBe(0);
    resident.forget("2/17/10/20");
    meshes.sync(resident);
    expect(meshes.isRefused("2/17/10/20")).toBe(false);
    meshes.dispose();
  });

  it("holds no mesh of a build that fails, and asks again at the next change of the layers", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const centre = normalise(vec3(0.3, 1, 0.2));
    const originM = vec3(centre.x * RADIUS_M, centre.y * RADIUS_M, centre.z * RADIUS_M);
    const warned = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    let asked = 0;
    const failing = {
      build: (): Promise<null> => {
        asked += 1;
        return Promise.reject(new Error("no builder"));
      },
      dispose: (): void => undefined,
    };
    const resident = new ResidentRocks();
    const key: PatchKey = { face: 2, level: 17, i: 10, j: 20 };
    resident.take("2/17/10/20", bakeOf(key, originM, rockLayerFrom(rocksAbout(3, 3, centre, 12))));
    const meshes = new RockMeshes(engine, failing, 32 * 1024 * 1024);
    meshes.sync(resident);
    await Promise.resolve();
    await Promise.resolve();
    expect([meshes.size, meshes.building, meshes.refused, warned.mock.calls.length]).toEqual([
      0, 0, 0, 1,
    ]);
    resident.take("2/17/11/20", bakeOf({ ...key, i: 11 }, originM, rockLayerFrom([])));
    meshes.sync(resident);
    expect(asked).toBe(2);
    meshes.dispose();
  });

  it("lets a patch's mesh go with its rocks, and drops a build that lands after its patch left", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const centre = normalise(vec3(0.3, 1, 0.2));
    const originM = vec3(centre.x * RADIUS_M, centre.y * RADIUS_M, centre.z * RADIUS_M);
    const resident = new ResidentRocks();
    const key: PatchKey = { face: 2, level: 17, i: 10, j: 20 };
    resident.take("2/17/10/20", bakeOf(key, originM, rockLayerFrom(rocksAbout(3, 3, centre, 12))));
    const meshes = new RockMeshes(engine, new InThreadRockPageBuilder(), 32 * 1024 * 1024);
    meshes.sync(resident);
    resident.forget("2/17/10/20");
    await Promise.resolve();
    await Promise.resolve();
    expect([meshes.size, meshes.building]).toEqual([0, 0]);
    resident.take("2/17/10/20", bakeOf(key, originM, rockLayerFrom(rocksAbout(3, 3, centre, 12))));
    meshes.sync(resident);
    await Promise.resolve();
    await Promise.resolve();
    expect(meshes.size).toBe(1);
    const used = meshes.pool.pressure().rootSlotsUsed;
    expect(used).toBeGreaterThan(0);
    resident.forget("2/17/10/20");
    meshes.sync(resident);
    expect([meshes.size, meshes.pool.pressure().rootSlotsUsed]).toEqual([0, 0]);
    meshes.dispose();
  });
});
