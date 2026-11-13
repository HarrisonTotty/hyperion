/**
 * The cluster path's buckets and counters (plan R14, R14.T1.b): each (pass, template) bucket's
 * `drawIndexedIndirect` arguments and its list's room, the draws made of them, and the counts read
 * back from them.
 */

import { describe, expect, it } from "vitest";

import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import type { BufferHandle, DrawItem } from "../engine/types";
import {
  BUCKET_ARGS_BYTES,
  CLUSTER_BASE_UNIFORM,
  ClusterBuckets,
  MAX_LIST_ENTRIES,
} from "./buckets";
import { ClusterStats, PASS_STATS_WORDS, passStats } from "./stats";
import { terrainClusterMesh } from "./templates";

/** Terrain's eight strided templates; its may-discard ones (R14.T1.g) are their own test's. */
const templates = terrainClusterMesh().templates.filter((t) => !t.mayDiscard);

/** A terrain view's room: 10 instances, 64 blocks and 32 strips each. */
const CAPACITIES = templates.map((t) => 10 * (t.shape === "terrain-block" ? 64 : 32));

/** A counting engine whose reads answer with `answer`. */
class AnsweringEngine extends CountingRenderEngine {
  answer = new Uint32Array();

  override readBuffer(_buffer?: BufferHandle): Promise<ArrayBuffer> {
    return Promise.resolve(this.answer.slice().buffer);
  }
}

const BASE: Omit<DrawItem, "mesh" | "indirect" | "instanceCount"> = {
  material: { kind: "material", name: "caster" },
  offsetFromCameraM: new Float32Array(3),
  uniforms: { bodyRotation: new Float32Array(16) },
  textures: {},
};

describe("the cluster path's buckets (plan R14, R14.T1.b)", () => {
  it("writes each bucket's arguments as its template's index count and first index, its count zero, and each pass's count of strides past τ′ after them zero (R14.T3.e)", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const buckets = new ClusterBuckets(engine, "test clusters", 2, templates, CAPACITIES);
    buckets.clear();
    const write = engine.writes.at(-1);
    const words = [...new Uint32Array(write?.data.slice().buffer ?? new ArrayBuffer(0))];
    const expected = [
      ...[0, 1].flatMap(() => templates.flatMap((t) => [t.indexCount, 0, t.firstIndex, 0, 0])),
      0,
      0,
    ];
    expect([write?.buffer, write?.offsetBytes, words, buckets.overWord]).toEqual([
      "test clusters arguments",
      0,
      expected,
      2 * templates.length * (BUCKET_ARGS_BYTES / 4),
    ]);
    expect([buckets.args.bytes, buckets.lists.bytes]).toEqual([
      2 * templates.length * BUCKET_ARGS_BYTES + 2 * 4,
      2 * 4 * 10 * (64 + 32) * 4,
    ]);
  });

  it("places each pass's buckets one after another, each template's list after the last's", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const buckets = new ClusterBuckets(engine, "test clusters", 3, templates, CAPACITIES);
    expect([buckets.place(0), buckets.place(2), buckets.templateOffsets]).toEqual([
      { bucketBase: 0, listBase: 0 },
      { bucketBase: 16, listBase: 2 * buckets.passEntries },
      [0, 640, 1280, 1920, 2560, 2880, 3200, 3520],
    ]);
  });

  it("draws a pass's buckets up to its stride cap, each from its arguments and its list's base", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const buckets = new ClusterBuckets(engine, "test clusters", 2, templates, CAPACITIES);
    const mesh = engine.createMesh(terrainClusterMesh().mesh);
    const items = buckets.drawItems(1, mesh, BASE, 1);
    expect(
      items.map((item) => [
        item.indirect?.offsetBytes,
        item.uniforms[CLUSTER_BASE_UNIFORM]?.[0],
        item.mesh,
      ]),
    ).toEqual([
      [8 * BUCKET_ARGS_BYTES, buckets.passEntries, mesh],
      [12 * BUCKET_ARGS_BYTES, buckets.passEntries + 2560, mesh],
    ]);
    expect(buckets.drawItems(1, mesh, BASE, 1)).toBe(items);
    expect(buckets.drawItems(1, mesh, BASE, 8)).toHaveLength(8);
  });

  it("draws a pass's may-discard buckets by its discard base, and none without one (R14.T1.g)", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const all = terrainClusterMesh().templates;
    const buckets = new ClusterBuckets(
      engine,
      "test clusters",
      2,
      all,
      all.map(() => 10),
    );
    const mesh = engine.createMesh(terrainClusterMesh().mesh);
    const discarding = { ...BASE, material: { kind: "material", name: "fin caster" } } as const;
    const items = buckets.drawItems(0, mesh, BASE, 1, discarding);
    expect(
      items.map((item) => [
        item.material.name,
        (item.indirect?.offsetBytes ?? 0) / BUCKET_ARGS_BYTES,
      ]),
    ).toEqual([
      ["caster", 0],
      ["caster", 4],
      ["fin caster", 8],
      ["fin caster", 9],
      ["fin caster", 10],
      ["fin caster", 11],
    ]);
    expect(buckets.drawItems(0, mesh, BASE, 1)).toHaveLength(2);
  });

  it("draws a pass's may-discard buckets alone where its base is none (R14.T2.c)", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const all = terrainClusterMesh().templates;
    const buckets = new ClusterBuckets(
      engine,
      "test clusters",
      2,
      all,
      all.map(() => 10),
    );
    const mesh = engine.createMesh(terrainClusterMesh().mesh);
    const discarding = { ...BASE, material: { kind: "material", name: "fin form" } } as const;
    expect(
      buckets
        .drawItems(0, mesh, null, 8, discarding)
        .map((item) => [item.material.name, (item.indirect?.offsetBytes ?? 0) / BUCKET_ARGS_BYTES]),
    ).toEqual([8, 9, 10, 11].map((t) => ["fin form", t]));
    expect(buckets.drawItems(0, mesh, null, 8, null)).toEqual([]);
  });

  it("keeps each of a pass's last four sets of draws, by mesh, bases and cap, the oldest let go first (R14.T2.c)", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const buckets = new ClusterBuckets(engine, "test clusters", 2, templates, CAPACITIES);
    const mesh = engine.createMesh(terrainClusterMesh().mesh);
    const ids = engine.createMesh({ ...terrainClusterMesh().mesh, name: "ids" });
    const base = (name: string): typeof BASE => ({
      ...BASE,
      material: { kind: "material", name },
    });
    const [lit, depth, raster, fourth, fifth] = ["lit", "depth", "raster", "4", "5"].map(base);
    const litItems = buckets.drawItems(1, mesh, lit ?? BASE, 1);
    const depthItems = buckets.drawItems(1, mesh, depth ?? BASE, 1);
    const rasterItems = buckets.drawItems(1, ids, raster ?? BASE, 1);
    // Each form a frame draws the pass by keeps its draws whatever the others drew meanwhile.
    expect([
      buckets.drawItems(1, mesh, lit ?? BASE, 1),
      buckets.drawItems(1, mesh, depth ?? BASE, 1),
      buckets.drawItems(1, ids, raster ?? BASE, 1),
    ]).toEqual([litItems, depthItems, rasterItems]);
    expect(buckets.drawItems(1, mesh, lit ?? BASE, 1)).toBe(litItems);
    expect(buckets.drawItems(1, ids, raster ?? BASE, 1)).toBe(rasterItems);
    expect(buckets.drawItems(1, mesh, raster ?? BASE, 1)).not.toBe(rasterItems);
    // A fourth and a fifth set let the oldest go: the lit form's is made again.
    buckets.drawItems(1, mesh, fourth ?? BASE, 1);
    buckets.drawItems(1, mesh, fifth ?? BASE, 1);
    const again = buckets.drawItems(1, mesh, lit ?? BASE, 1);
    expect([again === litItems, again]).toEqual([false, litItems]);
  });

  it("refuses capacities not one a template, and lists past an exact f32 base", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    expect(() => new ClusterBuckets(engine, "test", 1, templates, [1])).toThrow(RangeError);
    expect(
      () =>
        new ClusterBuckets(
          engine,
          "test",
          2,
          templates,
          templates.map(() => MAX_LIST_ENTRIES / 8),
        ),
    ).toThrow(RangeError);
  });

  it("releases its lists and arguments once", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const before = engine.live.buffers;
    const buckets = new ClusterBuckets(engine, "test clusters", 1, templates, CAPACITIES);
    buckets.dispose();
    buckets.dispose();
    expect(engine.live.buffers).toBe(before);
  });
});

describe("the cluster path's counters (plan R14, R14.T1.b)", () => {
  it("reads each pass's clusters, triangles and clusters by stride from its reduction's words", () => {
    const words = Uint32Array.from([6, 3 * 128 + 2 + 2 * 16, 5, 0, 0, 1, ...Array(6).fill(0)]);
    expect([PASS_STATS_WORDS, passStats(words, ["cascade 1", "cascade 2"])]).toEqual([
      6,
      [
        {
          label: "cascade 1",
          clusters: 6,
          triangles: 3 * 128 + 2 + 2 * 16,
          byStride: [5, 0, 0, 1],
        },
        { label: "cascade 2", clusters: 0, triangles: 0, byStride: [0, 0, 0, 0] },
      ],
    ]);
  });

  it("reduces the counts by its bit-exact kernel, each template's triangles and stride its uniforms, and gives the read's answer as its latest", async () => {
    const engine = new AnsweringEngine(await fakeDevice());
    engine.answer = Uint32Array.from([5, 5 * 128, 5, 0, 0, 0]);
    const buckets = new ClusterBuckets(engine, "test clusters", 1, templates, CAPACITIES);
    const stats = new ClusterStats(engine, "test clusters");
    expect(stats.latest()).toBeNull();
    stats.sample(buckets, templates, ["cascade 1"]);
    await Promise.resolve();
    await Promise.resolve();
    const dispatched = engine.dispatched.at(-1);
    const params = dispatched?.bindings.uniforms["params"] ?? new Uint32Array();
    expect([
      dispatched?.kernel,
      dispatched?.bindings.buffers["args"],
      [...params.subarray(0, 2)],
      [...params.subarray(4, 12)],
      [...params.subarray(20, 28)],
      stats.latest()?.[0],
    ]).toEqual([
      "cluster stats reduction",
      buckets.args,
      [1, 8],
      [128, 32, 8, 2, 16, 8, 4, 2],
      [0, 1, 2, 3, 0, 1, 2, 3],
      { label: "cascade 1", clusters: 5, triangles: 5 * 128, byStride: [5, 0, 0, 0] },
    ]);
    expect(engine.kernelPairs.at(-1)?.readback).toBe("bit-exact");
    stats.dispose();
    buckets.dispose();
  });
});
