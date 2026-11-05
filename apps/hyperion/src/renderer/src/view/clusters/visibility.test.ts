/**
 * The visibility buffer's ID raster (plan R14, R14.T2.c): the ID's packing, the de-indexed mesh the
 * raster draws without `primitive-index` against the cluster mesh the `primitive_index` path draws,
 * and the views' index space.
 */

import { describe, expect, it } from "vitest";

import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import { ID_NONE } from "../engine/idTarget";
import { MAX_LIST_ENTRIES } from "./buckets";
import { terrainClusterMesh } from "./templates";
import {
  DEINDEXED_TRIANGLE_STEP,
  deindexedClusterMesh,
  packVisibilityId,
  unpackVisibilityId,
  VISIBILITY_TRIANGLE_BITS,
  VISIBILITY_TRIANGLES,
  VisibilityBuffer,
  visibleConsumerRanges,
} from "./visibility";

/** `terrainClusterId.wgsl`'s de-indexed ID of a vertex: its entry's index and its z's triangle. */
function deindexedIdOf(visible: number, z: number): number {
  const packed = Math.floor(z + 0.5);
  return (
    ((visible << VISIBILITY_TRIANGLE_BITS) | Math.floor(packed / DEINDEXED_TRIANGLE_STEP)) >>> 0
  );
}

/** `terrainClusterIdPrimitive.wgsl`'s ID: the entry's index and the primitive's index in the draw. */
function primitiveIdOf(visible: number, primitive: number): number {
  return ((visible << VISIBILITY_TRIANGLE_BITS) | primitive) >>> 0;
}

describe("the visibility buffer's ID (plan R14, R14.T2.c)", () => {
  it("packs the visible-cluster index above the triangle and unpacks it", () => {
    const cases = [
      [0, 0],
      [1, 127],
      [12_345, 64],
      [MAX_LIST_ENTRIES - 1, VISIBILITY_TRIANGLES - 1],
    ] as const;
    expect(
      cases.map(([visible, triangle]) => unpackVisibilityId(packVisibilityId(visible, triangle))),
    ).toEqual(cases.map(([visible, triangle]) => ({ visible, triangle })));
    expect(packVisibilityId(3, 5)).toBe(3 * 128 + 5);
  });

  it("never packs to none, which reads back as no cluster", () => {
    expect(packVisibilityId(MAX_LIST_ENTRIES - 1, VISIBILITY_TRIANGLES - 1)).toBeLessThan(ID_NONE);
    expect(unpackVisibilityId(ID_NONE)).toBeNull();
  });

  it("refuses an index or a triangle no ID holds", () => {
    for (const [visible, triangle] of [
      [MAX_LIST_ENTRIES, 0],
      [-1, 0],
      [0, VISIBILITY_TRIANGLES],
      [0.5, 0],
    ] as const) {
      expect(() => packVisibilityId(visible, triangle)).toThrow(RangeError);
    }
  });
});

describe("the ID raster's de-indexed mesh (plan R14, R14.T2.c)", () => {
  const cluster = terrainClusterMesh();
  const deindexed = deindexedClusterMesh(cluster, "ids");
  const indices = cluster.mesh.indices ?? new Uint32Array();

  it("gives each index its own vertex, the one the index named, with its triangle above its code", () => {
    const wrong: string[] = [];
    for (const template of cluster.templates) {
      for (let k = 0; k < template.indexCount; k += 1) {
        const i = template.firstIndex + k;
        const v = indices[i] ?? -1;
        const own = Array.from(cluster.mesh.positions.subarray(3 * v, 3 * v + 3));
        const [x, y, z] = Array.from(deindexed.positions.subarray(3 * i, 3 * i + 3));
        const triangle = Math.floor(k / 3);
        if (
          deindexed.indices?.[i] !== i ||
          x !== own[0] ||
          y !== own[1] ||
          z !== (own[2] ?? 0) + DEINDEXED_TRIANGLE_STEP * triangle
        ) {
          wrong.push(`${template.shape} at stride ${String(template.stride)}, index ${String(k)}`);
        }
      }
    }
    expect(wrong).toEqual([]);
    expect(deindexed.positions).toHaveLength(3 * indices.length);
  });

  it("names every triangle of a bucket draw as the primitive_index path does, every vertex of it alike", () => {
    // A bucket draw is one template's index range an instance, one cluster an instance, so the
    // primitive index of its k-th triangle is k (W3C WGSL's `primitive_index`, recalled).
    const visible = 4_321;
    const differ: string[] = [];
    for (const template of cluster.templates) {
      for (let k = 0; k < template.triangles; k += 1) {
        const want = primitiveIdOf(visible, k);
        for (let corner = 0; corner < 3; corner += 1) {
          const i = template.firstIndex + 3 * k + corner;
          const got = deindexedIdOf(visible, deindexed.positions[3 * i + 2] ?? Number.NaN);
          if (got !== want) {
            differ.push(`${template.shape} triangle ${String(k)} corner ${String(corner)}`);
          }
        }
      }
    }
    expect(differ).toEqual([]);
  });

  it("keeps every vertex's lit-mesh code below the triangle's step, as the raster's decode drops it", () => {
    const codes = new Set<number>();
    for (let i = 0; i < indices.length; i += 1) {
      codes.add(Math.floor((deindexed.positions[3 * i + 2] ?? 0) + 0.5) % DEINDEXED_TRIANGLE_STEP);
    }
    expect([...codes].toSorted((a, b) => a - b)).toEqual([0, 1]);
    expect(Math.max(...cluster.templates.map((t) => t.triangles))).toBeLessThanOrEqual(
      VISIBILITY_TRIANGLES,
    );
  });

  it("refuses a mesh with no indices, or with attributes beside its positions", () => {
    expect(() =>
      deindexedClusterMesh({ ...cluster, mesh: { ...cluster.mesh, indices: null } }, "ids"),
    ).toThrow(/no indices/);
    const normal = { data: new Float32Array(3), size: 3 } as const;
    expect(() =>
      deindexedClusterMesh(
        { ...cluster, mesh: { ...cluster.mesh, attributes: { normal } } },
        "ids",
      ),
    ).toThrow(/attributes beside its positions/);
  });
});

describe("the views' visible-cluster index space (plan R14, R14.T2.c; Design note 9)", () => {
  it("gives terrain's lists, all its passes', the space's first range, from 0, for a later consumer's after it", () => {
    expect(visibleConsumerRanges(70_000)).toEqual([
      { consumer: "terrain", first: 0, entries: 70_000 },
    ]);
    expect(() => visibleConsumerRanges(MAX_LIST_ENTRIES + 1)).toThrow(RangeError);
  });
});

describe("the view's visibility target (plan R14, R14.T2.c)", () => {
  it("is an r32uint ID target drawn with its scene target's depth, at its size, and follows it", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    const scene = engine.createRenderTarget({
      name: "view:hdr",
      size: { widthPx: 64, heightPx: 32 },
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
    const buffer = new VisibilityBuffer(engine, "view", scene, { widthPx: 64, heightPx: 32 });
    const made = engine.textureSpecs.at(-1);
    scene.resize({ widthPx: 32, heightPx: 16 });
    buffer.resize({ widthPx: 32, heightPx: 16 });
    expect([made?.format, made?.size, buffer.target.name]).toEqual([
      "r32uint",
      [64, 32],
      "view:ids",
    ]);
    // Its IDs and its depth follow the targets' textures across a resize.
    expect(buffer.ids).toBe(buffer.target.colour);
    expect(buffer.target.depth).toBe(scene.depth);
    buffer.dispose();
  });
});
