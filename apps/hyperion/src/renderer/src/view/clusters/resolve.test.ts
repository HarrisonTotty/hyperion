/**
 * The visibility buffer's resolve's twins (plan R14, R14.T2.d): an entry's template from its index,
 * a template triangle's corners against the cluster mesh, and the barycentrics of a pixel from three
 * clip positions against the point that projects to it.
 */

import { describe, expect, it } from "vitest";

import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import { ClusterBuckets } from "./buckets";
import {
  clipBarycentrics,
  type ClipPosition,
  RESOLVE_TEMPLATES,
  resolveUniforms,
  snappedClip,
  templateTriangle,
  visibleTemplate,
  VISIBILITY_IDS_TEXTURE,
  withVisibilityIds,
} from "./resolve";
import resolveTerrainWgsl from "./resolveTerrain.wgsl?raw";
import { terrainClusterMesh } from "./templates";
import { TERRAIN_LIGHT_BUFFER_ROWS } from "../terrain/terrainMaterial";

/** Terrain's cluster mesh and templates, as the raster draws them. */
const CLUSTER = terrainClusterMesh();

describe("the resolve's template of an entry (plan R14, R14.T2.d)", () => {
  it("finds every entry's bucket in every pass of a view's buckets, its templates of any room", async () => {
    const engine = new CountingRenderEngine(await fakeDevice());
    // Rooms of every kind, an empty one among them, so that two templates share an offset.
    const capacities = CLUSTER.templates.map((_, k) => (k === 2 ? 0 : 3 + k));
    const buckets = new ClusterBuckets(engine, "test", 7, CLUSTER.templates, capacities);
    const uniforms = resolveUniforms(buckets, 8);
    const offsets = [...uniforms.resolveTemplates0, ...uniforms.resolveTemplates1];
    const wrong: string[] = [];
    for (let pass = 0; pass < buckets.passes; pass += 1) {
      for (let t = 0; t < RESOLVE_TEMPLATES; t += 1) {
        const first = buckets.place(pass).listBase + (buckets.templateOffsets[t] ?? 0);
        for (let k = 0; k < (capacities[t] ?? 0); k += 1) {
          const found = visibleTemplate(first + k, buckets.passEntries, offsets);
          if (found !== t) {
            wrong.push(
              `pass ${String(pass)} template ${String(t)} entry ${String(k)}: ${String(found)}`,
            );
          }
        }
      }
    }
    expect(wrong).toEqual([]);
    expect([...uniforms.resolveLists]).toEqual([
      buckets.passEntries,
      0,
      buckets.passes * buckets.passEntries,
      256,
    ]);
  });
});

describe("the resolve's triangle corners (plan R14, R14.T2.d)", () => {
  it("are each opaque template triangle's, in its index order, as the cluster mesh holds them", () => {
    const indices = CLUSTER.mesh.indices ?? new Uint32Array();
    const differ: string[] = [];
    for (const template of CLUSTER.templates.filter((t) => !t.mayDiscard)) {
      for (let k = 0; k < template.triangles; k += 1) {
        const corners = templateTriangle(template, k);
        for (let c = 0; c < 3; c += 1) {
          const v = indices[template.firstIndex + 3 * k + c] ?? -1;
          const want = Array.from(CLUSTER.mesh.positions.subarray(3 * v, 3 * v + 3));
          if (JSON.stringify(corners[c]) !== JSON.stringify(want)) {
            differ.push(`${template.shape} ${String(template.stride)} triangle ${String(k)}`);
          }
        }
      }
    }
    expect(differ).toEqual([]);
    expect(CLUSTER.templates.slice(0, RESOLVE_TEMPLATES).every((t) => !t.mayDiscard)).toBe(true);
  });

  it("refuses a may-discard template's triangle and one past a template's", () => {
    const block = CLUSTER.templates[0];
    const fin = CLUSTER.templates.find((t) => t.mayDiscard);
    if (block === undefined || fin === undefined) {
      throw new Error("the cluster mesh has its templates");
    }
    expect(() => templateTriangle(block, block.triangles)).toThrow(RangeError);
    expect(() => templateTriangle(fin, 0)).toThrow(RangeError);
  });
});

/** `p` (x, y, z, 1) through the column-major 4 × 4 `m`. */
function transform(m: ReadonlyArray<number>, p: readonly [number, number, number]): ClipPosition {
  const at = (row: number): number =>
    (m[row] ?? 0) * p[0] + (m[4 + row] ?? 0) * p[1] + (m[8 + row] ?? 0) * p[2] + (m[12 + row] ?? 0);
  return [at(0), at(1), at(2), at(3)];
}

/** A reversed-Z infinite perspective of 60° horizontal field at a 16 : 9 aspect, near 0.1 m. */
const PROJECTION = (() => {
  const f = 1 / Math.tan(Math.PI / 6);
  return [f, 0, 0, 0, 0, (f * 16) / 9, 0, 0, 0, 0, 0, -1, 0, 0, 0.1, 0];
})();

describe("the resolve's barycentrics (plan R14, R14.T2.d)", () => {
  it("weigh the three corners to the place on the triangle that projects to the pixel, perspective-correct", () => {
    let largest = 0;
    let seed = 7;
    const random = (): number => {
      seed = (seed * 16_807) % 2_147_483_647;
      return seed / 2_147_483_647;
    };
    for (let n = 0; n < 200; n += 1) {
      // A triangle 5 m to 5 km in front, of any slant, and a point inside it.
      const depth = 5 * 10 ** (3 * random());
      const corner = (): readonly [number, number, number] => [
        (random() - 0.5) * depth,
        (random() - 0.5) * depth,
        -depth * (0.5 + random()),
      ];
      const places = [corner(), corner(), corner()] as const;
      const w = [random(), random(), random()];
      const sum = (w[0] ?? 0) + (w[1] ?? 0) + (w[2] ?? 0);
      const mean = (axis: number): number =>
        places.reduce((s, p, k) => s + (w[k] ?? 0) * (p[axis] ?? 0), 0) / sum;
      const inside = [mean(0), mean(1), mean(2)] as const;
      const clips = places.map((p) => transform(PROJECTION, p));
      const projected = transform(PROJECTION, inside);
      const ndc = [projected[0] / projected[3], projected[1] / projected[3]] as const;
      const [c0, c1, c2] = clips;
      if (c0 === undefined || c1 === undefined || c2 === undefined) {
        throw new Error("three corners");
      }
      const b = clipBarycentrics(c0, c1, c2, ndc);
      for (let axis = 0; axis < 3; axis += 1) {
        const got = places.reduce((s, p, k) => s + (b[k] ?? 0) * (p[axis] ?? 0), 0);
        largest = Math.max(largest, Math.abs(got - (inside[axis] ?? 0)) / depth);
      }
    }
    expect(largest).toBeLessThan(1e-9);
  });

  it("sum to 1 and give a corner its own place", () => {
    const c0: ClipPosition = [-1, -1, 0.1, 2];
    const c1: ClipPosition = [2, -1, 0.1, 3];
    const c2: ClipPosition = [-1, 3, 0.1, 4];
    const b = clipBarycentrics(c0, c1, c2, [c1[0] / c1[3], c1[1] / c1[3]]);
    expect(b.map((x) => Math.round(x * 1e12) / 1e12)).toEqual([0, 1, 0]);
  });

  it("take a pixel's centre just outside the triangle on its edge, however near edge-on it is seen", () => {
    // A triangle whose third corner lies almost on the line through the first two, seen nearly
    // edge-on, and a point a hair beyond its long edge, as a sub-pixel grid puts a pixel's centre.
    const c0: ClipPosition = [-1, 0, 0.1, 1];
    const c1: ClipPosition = [4, 0, 0.1, 5];
    const c2: ClipPosition = [0, 1e-6, 0.1, 2];
    const b = clipBarycentrics(c0, c1, c2, [0.1, -1e-4]);
    expect([b.every((x) => x >= 0), Math.abs(b[0] + b[1] + b[2] - 1) < 1e-12, b[2]]).toEqual([
      true,
      true,
      0,
    ]);
  });

  it("take a pixel just past a sliver's far edge on that edge, its plane's horizon just beyond", () => {
    // A 10 m sliver 1 km off whose plane passes 1 mm from the eye, and a pixel 1/16 px (of 1,080)
    // past its far edge, beyond which the weights' sum changes sign.
    const c0: ClipPosition = [0, -0.001, 0.1, 1000];
    const c1: ClipPosition = [-5, -0.001, 0.1, 1010];
    const c2: ClipPosition = [5, -0.001, 0.1, 1010];
    const ndc = [0, -0.001 / 1010 + 2 / 1080 / 16] as const;
    expect(clipBarycentrics(c0, c1, c2, ndc).map((x) => Math.round(x * 1e6) / 1e6)).toEqual([
      0, 0.5, 0.5,
    ]);
  });

  it("give the first corner alone for a triangle seen edge-on", () => {
    const c0: ClipPosition = [-1, 0, 0.1, 1];
    const c1: ClipPosition = [0, 0, 0.1, 2];
    const c2: ClipPosition = [1, 0, 0.1, 3];
    expect(clipBarycentrics(c0, c1, c2, [0, 0])).toEqual([1, 0, 0]);
  });
});

describe("the resolve's corners on the rasteriser's sub-pixel grid (plan R14, R14.T2.d)", () => {
  const SIZE = { widthPx: 64, heightPx: 48 } as const;

  it("move each corner to the nearest cell of its grid, its depth and w kept", () => {
    // At (10.30, 20.45) px from the top left, w 3: the nearest sixteenths are (10.3125, 20.4375).
    const w = 3;
    const corner: ClipPosition = [(10.3 / 32 - 1) * w, (1 - 20.45 / 24) * w, 0.2, w];
    const [x, y, z, sw] = snappedClip(corner, SIZE, 16);
    const px = [(0.5 + (0.5 * x) / sw) * 64, (0.5 - (0.5 * y) / sw) * 48];
    expect([px.map((v) => Math.round(v * 1e9) / 1e9), z, sw]).toEqual([[10.3125, 20.4375], 0.2, w]);
  });

  it("snap a corner at a half cell to the even cell, as WGSL's round does", () => {
    // At 10.03125 px, between the sixteenths 160 and 161: the even one, 10 px.
    const corner: ClipPosition = [10.03125 / 32 - 1, 0, 0.2, 1];
    const [x] = snappedClip(corner, SIZE, 16);
    expect((0.5 + 0.5 * x) * 64).toBe(10);
  });

  it("keep a corner behind the eye whole", () => {
    const behind: ClipPosition = [0.3, -0.2, 0.1, -2];
    expect(snappedClip(behind, SIZE, 16)).toBe(behind);
  });
});

describe("the resolve's draw and frame (plan R14, R14.T2.d)", () => {
  it("binds the view's IDs beside the draw's own textures", () => {
    const draw = {
      mesh: { kind: "mesh", name: "screen" },
      material: { kind: "material", name: "resolve" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: { normals: { kind: "texture", name: "normals" } },
    } as const;
    const ids = { kind: "texture", name: "ids" } as const;
    expect(withVisibilityIds(draw, ids).textures).toEqual({
      normals: draw.textures.normals,
      [VISIBILITY_IDS_TEXTURE]: ids,
    });
  });

  it("reads its contacts after the lights' rows, as the resolve frame holds them", () => {
    expect(resolveTerrainWgsl).toContain(
      `const RESOLVE_CONTACTS_TEXEL : u32 = ${String(TERRAIN_LIGHT_BUFFER_ROWS)}u;`,
    );
  });
});
