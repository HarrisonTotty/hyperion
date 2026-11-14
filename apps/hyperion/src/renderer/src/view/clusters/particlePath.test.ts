/**
 * A view's particle path and the instances culled above the cluster cull (plan R14, R14.T5.d): the
 * buffers each makes, the dispatches each frame takes, folded, and the bucket draws, on a counting
 * engine; and the test scene's camera pass of a submission's frustum.
 */

import { describe, expect, it } from "vitest";

import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import { declaredStructLayout } from "../engine/webgpu/uniforms";
import { frustumOf } from "../terrain/cull";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { CLUSTER_BASE_UNIFORM } from "./buckets";
import { ClusterConsumerPath } from "./consumer";
import { CLUSTER_CULL_KERNEL } from "./cull";
import {
  INSTANCE_COMPACT_KERNEL,
  INSTANCE_CULL_KERNEL,
  INSTANCE_FOLD_KERNEL,
  foldTail,
} from "./instanceCull";
import {
  cameraCullPassOf,
  InstanceScene,
  PARTICLE_TEST_MATERIAL,
  TEST_PLACE_PARAMS_BYTES,
  viewAxisOf,
} from "./instanceScene";
import instanceTestPlaceWgsl from "./instanceTestPlace.wgsl?raw";
import { ParticlePath } from "./particlePath";
import {
  PARTICLE_INSTANCE_BYTES,
  packParticleInstance,
  sphereParticleTemplates,
} from "./particles";
import { meshTemplate } from "./templates";

const OPEN = {
  kind: "camera",
  planes: [],
  horizon: null,
  order: "appended",
  toleranceRad: 0.001,
  strideCap: 1,
} as const;

describe("a view's particle path (plan R14, R14.T5.d)", () => {
  it("makes its instances, count, lists and arguments, the lists every instance once a template a pass", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ParticlePath(engine, "pebbles", sphereParticleTemplates(), 1000, 2);
    expect([
      path.instances.bytes,
      path.count.bytes,
      path.buckets.lists.bytes,
      path.buckets.capacities,
    ]).toEqual([1000 * PARTICLE_INSTANCE_BYTES, 16, 2 * 4 * 1000 * 4, [1000, 1000, 1000, 1000]]);
    path.dispose();
    expect(engine.live.buffers).toBe(0);
  });

  it("culls once a frame over its capacity, folded, after its arguments are cleared, its passes all entered", async () => {
    // 4,194,304 instances, the most four templates' lists hold for a pass: 65,536 workgroups.
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ParticlePath(engine, "pebbles", sphereParticleTemplates(), 4_194_304, 1);
    const kernel = engine.createCompute(INSTANCE_CULL_KERNEL);
    path.cull(kernel, { passes: [OPEN], viewAxis: { x: 0, y: 0, z: -1 } });
    const dispatch = engine.dispatched.at(-1);
    const params = new Uint32Array(
      dispatch?.bindings.uniforms["params"]?.buffer ?? new ArrayBuffer(96),
    );
    expect([
      dispatch?.kernel,
      dispatch?.pass,
      dispatch?.workgroups,
      Object.keys(dispatch?.bindings.buffers ?? {}).toSorted(),
      engine.writes.at(-1)?.buffer,
      [params[0], params[1], params[2], params[3], params[20]],
    ]).toEqual([
      "instance cull",
      "instance cull",
      [32_768, 2, 1],
      ["args", "instance_count", "instance_words", "lists"],
      "pebbles arguments",
      [4_194_304, 8, 3, 1, 1],
    ]);
    expect(() => {
      path.cull(kernel, { passes: [OPEN, OPEN], viewAxis: { x: 0, y: 0, z: -1 } });
    }).toThrow(RangeError);
  });

  it("holds at most 4,194,304 instances a pass at four templates, the views check's most (preload's MAX_INSTANCE_PROBE)", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const templates = sphereParticleTemplates();
    expect(new ParticlePath(engine, "most", templates, 4_194_304, 1).capacity).toBe(4_194_304);
    expect(() => new ParticlePath(engine, "past", templates, 4_194_305, 1)).toThrow(RangeError);
  });

  it("writes the CPU's instances and their count through the queue, refusing more than it holds", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ParticlePath(engine, "pebbles", sphereParticleTemplates(), 4, 1);
    const data = new ArrayBuffer(3 * PARTICLE_INSTANCE_BYTES);
    for (let k = 0; k < 3; k += 1) {
      packParticleInstance(new DataView(data), k * PARTICLE_INSTANCE_BYTES, {
        centreM: { x: k, y: 0, z: -10 },
        radiusM: 0.5,
        rotation: IDENTITY_QUATERNION,
        colour: 0xffffffff,
        tag: k,
      });
    }
    path.writeInstances(data, 3);
    expect(engine.writes.slice(-2).map((w) => [w.buffer, w.bytes])).toEqual([
      ["pebbles instances", 3 * PARTICLE_INSTANCE_BYTES],
      ["pebbles count", 16],
    ]);
    expect([
      ...new Uint32Array(engine.writes.at(-1)?.data.slice().buffer ?? new ArrayBuffer(0)),
    ]).toEqual([3, 0, 0, 0]);
    expect(() => {
      path.writeInstances(new ArrayBuffer(5 * PARTICLE_INSTANCE_BYTES), 5);
    }).toThrow(RangeError);
  });

  it("draws a pass's buckets, one a template, each from its arguments, with its instances and lists bound", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const templates = sphereParticleTemplates();
    const path = new ParticlePath(engine, "pebbles", templates, 100, 2);
    const mesh = engine.createMesh(templates.mesh);
    const base = {
      material: engine.createMaterial(PARTICLE_TEST_MATERIAL),
      offsetFromCameraM: new Float32Array(3),
      uniforms: { light: new Float32Array(4) },
      textures: {},
    };
    const items = path.drawItems(1, mesh, base);
    expect(
      items.map((item) => [
        item.indirect?.offsetBytes,
        item.uniforms[CLUSTER_BASE_UNIFORM]?.[0],
        item.storageBuffers?.["particleInstances"]?.name,
        item.storageBuffers?.["particleLists"]?.name,
      ]),
    ).toEqual(
      [0, 1, 2, 3].map((t) => [(4 + t) * 20, 400 + 100 * t, "pebbles instances", "pebbles lists"]),
    );
    expect(path.drawItems(1, mesh, base)).toBe(items);
  });
});

/** A mesh consumer of 32 B instances: its origin, page, then its radius at word 4. */
const MESH_CONSUMER = {
  name: "rocks",
  templates: [meshTemplate()],
  clustersPerPage: 8,
  firstShape: 2,
  templateClusters: [8],
  instanceBytes: 32,
  vertexWgsl: "",
};

describe("instances culled above the cluster cull (plan R14, R14.T5.d)", () => {
  it("compacts the instances, folds their count and culls their clusters over them, three dispatches in order", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ClusterConsumerPath(engine, MESH_CONSUMER);
    const kernels = {
      compact: engine.createCompute(INSTANCE_COMPACT_KERNEL),
      fold: engine.createCompute(INSTANCE_FOLD_KERNEL),
      cull: engine.createCompute(CLUSTER_CULL_KERNEL),
    };
    const buffer = (name: string, bytes: number) =>
      engine.createBuffer({ name, bytes, usage: 0, category: "geometry-clusters" });
    path.cullInstances(
      kernels,
      {
        records: buffer("records", 4096),
        instances: buffer("instances", 70_000 * 32),
        count: buffer("count", 16),
        capacity: 70_000,
        radiusWord: 4,
        passes: [OPEN],
        passMask: 1,
        rotationColumns: [
          { x: 1, y: 0, z: 0 },
          { x: 0, y: 1, z: 0 },
          { x: 0, y: 0, z: 1 },
        ],
        viewAxis: { x: 0, y: 0, z: -1 },
      },
      "rock cull",
    );
    const [compact, fold, cull] = engine.dispatched.slice(-3);
    const places = 70_000 + foldTail(70_000);
    expect([
      [compact?.kernel, compact?.pass, compact?.workgroups],
      [fold?.kernel, fold?.pass, fold?.workgroups, [...(fold?.bindings.uniforms["params"] ?? [])]],
      [cull?.kernel, cull?.pass, cull?.workgroups, cull?.bindings.buffers["instance_words"]?.name],
      new Uint32Array(cull?.bindings.uniforms["params"]?.buffer ?? new ArrayBuffer(64))[12],
      places,
    ]).toEqual([
      ["instance compact", "rock cull instances", [1094, 1, 1]],
      ["instance fold", "rock cull instances", [1, 1, 1], [70_000, places, 0, 0]],
      [
        "cluster cull",
        "rock cull",
        { buffer: path.survivors, offsetBytes: 4 },
        "rocks compacted instances",
      ],
      places,
      70_002,
    ]);
    expect(path.compactedInstances?.bytes).toBe(places * 32);
    path.dispose();
  });

  it("keeps its compacted buffers while the capacity fits them, makes them again where short, and releases them", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ClusterConsumerPath(engine, MESH_CONSUMER);
    const kernel = engine.createCompute(CLUSTER_CULL_KERNEL);
    const kernels = { compact: kernel, fold: kernel, cull: kernel };
    const handle = engine.createBuffer({ name: "b", bytes: 64, usage: 0, category: "other" });
    const cullAt = (capacity: number): number | undefined => {
      path.cullInstances(
        kernels,
        {
          records: handle,
          instances: handle,
          count: handle,
          capacity,
          radiusWord: 4,
          passes: [OPEN],
          passMask: 1,
          rotationColumns: [
            { x: 1, y: 0, z: 0 },
            { x: 0, y: 1, z: 0 },
            { x: 0, y: 0, z: 1 },
          ],
          viewAxis: { x: 0, y: 0, z: -1 },
        },
        "cull",
      );
      return path.compactedInstances?.bytes;
    };
    const sizes = [cullAt(70_000), cullAt(1000), cullAt(140_000)];
    const live = engine.live.buffers;
    path.dispose();
    expect([sizes, live - engine.live.buffers]).toEqual([
      [(70_000 + 2) * 32, (70_000 + 2) * 32, (140_000 + 3) * 32],
      // Its buckets' lists and arguments, its inputs and marks, and its three compacted buffers.
      7,
    ]);
  });

  it("refuses a radius word on the page's or past the record", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ClusterConsumerPath(engine, MESH_CONSUMER);
    const handle = engine.createBuffer({ name: "b", bytes: 64, usage: 0, category: "other" });
    const kernel = engine.createCompute(CLUSTER_CULL_KERNEL);
    const frame = {
      records: handle,
      instances: handle,
      count: handle,
      capacity: 1,
      radiusWord: 3,
      passes: [OPEN],
      passMask: 1,
      rotationColumns: [
        { x: 1, y: 0, z: 0 },
        { x: 0, y: 1, z: 0 },
        { x: 0, y: 0, z: 1 },
      ] as const,
      viewAxis: { x: 0, y: 0, z: -1 },
    };
    const kernels = { compact: kernel, fold: kernel, cull: kernel };
    expect(() => {
      path.cullInstances(kernels, frame, "cull");
    }).toThrow(RangeError);
    expect(() => {
      path.cullInstances(kernels, { ...frame, radiusWord: 8 }, "cull");
    }).toThrow(RangeError);
  });

  it("folds the plain cull's workgroups, one an instance, past 65,535 instances", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const path = new ClusterConsumerPath(engine, MESH_CONSUMER);
    const handle = engine.createBuffer({ name: "b", bytes: 64, usage: 0, category: "other" });
    path.cull(
      engine.createCompute(CLUSTER_CULL_KERNEL),
      {
        records: handle,
        instances: handle,
        count: 100_000,
        inputs: new Uint32Array(200_000),
        passes: [OPEN],
        rotationColumns: [
          { x: 1, y: 0, z: 0 },
          { x: 0, y: 1, z: 0 },
          { x: 0, y: 0, z: 1 },
        ],
        viewAxis: { x: 0, y: 0, z: -1 },
      },
      "cull",
    );
    expect(engine.dispatched.at(-1)?.workgroups).toEqual([50_000, 2, 1]);
  });
});

/** Whether two numbers agree to a millionth. */
const close = (a: number, b: number): boolean => Math.abs(a - b) < 1e-6;

describe("the instance path's test scene (plan R14, R14.T5.d)", () => {
  it("forms a submission's frustum as the camera's, its planes R05's", () => {
    // The harness's projection: a 90° vertical field, reversed-Z, at an aspect of 2.
    const near = 0.1;
    const projection = new Float32Array([0.5, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, near, 0]);
    const identity = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    const pass = cameraCullPassOf(identity, projection, 200, 1);
    const r05 = frustumOf({
      orientation: IDENTITY_QUATERNION,
      fovXRad: 2 * Math.atan(2),
      viewport: { widthPx: 200, heightPx: 100 },
    });
    const matched = r05.planes.filter((plane) =>
      pass.planes.some(
        (p) =>
          close(p.normal.x, plane.normal.x) &&
          close(p.normal.y, plane.normal.y) &&
          close(p.normal.z, plane.normal.z) &&
          close(p.offsetM, plane.offsetM === 0 ? 0 : -near),
      ),
    );
    expect([pass.planes.length, matched.length, pass.toleranceRad, viewAxisOf(identity)]).toEqual([
      5,
      5,
      1 / 50,
      { x: -0, y: -0, z: -1 },
    ]);
  });

  it("lays the field's placement out as its kernel's PlaceParams", () => {
    const layout = declaredStructLayout(instanceTestPlaceWgsl, "PlaceParams");
    expect([layout?.members.map((m) => [m.name, m.offsetBytes]), layout?.sizeBytes]).toEqual([
      [
        ["viewX", 0],
        ["viewY", 16],
        ["viewZ", 32],
        ["ranges", 48],
        ["counts", 64],
      ],
      TEST_PLACE_PARAMS_BYTES,
    ]);
  });

  it("places, culls and draws its field each frame: two dispatches and four bucket draws", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const scene = new InstanceScene(engine, "test field", 1000);
    const identity = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    const projection = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, 0.1, 0]);
    const draws = scene.frame({ viewRotation: identity, projection, widthPx: 64 });
    expect([
      engine.dispatched.slice(-2).map((d) => [d.kernel, d.workgroups]),
      draws.length,
    ]).toEqual([
      [
        ["instance test place", [16, 1, 1]],
        ["instance cull", [16, 1, 1]],
      ],
      4,
    ]);
    scene.dispose();
    expect([engine.live.buffers, engine.live.materials, engine.live.meshes]).toEqual([0, 0, 0]);
  });
});
