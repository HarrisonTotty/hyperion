/**
 * The shading normal's morph at a coarse–fine edge (plan R10, R10.T8.c; decision-r10-t16-leans
 * §16), on the real module's bakes of the test planet: a fine patch at k = 1 shades as its parent
 * along the edge it shares with its parent's neighbour, which shades with its own tile at k = 0.
 */

import { beforeAll, describe, expect, it } from "vitest";

import { dot, type Vec3 } from "../../geometry/vec3";
import {
  bakePatch,
  initSync,
  NormalScale,
  Ridges,
  VertexPath,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { memoised } from "../../test/memo";
import { parentQuadrant } from "./gpu/uniforms";
import { parentKey, type PatchKey, patchKeyString } from "./patchKey";
import { type NormalsTile, shadingNormal, tileNormalAt } from "./shadingNormal";
import { packNormals } from "./workers/heightBake";

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/** The two settings' normals: the low setting's at the mesh's resolution, the high's twice it. */
const SCALES = [
  { name: "low", scale: NormalScale.Mesh, samples: 65 },
  { name: "high", scale: NormalScale.Double, samples: 129 },
] as const;

type Scale = (typeof SCALES)[number];

/** A real bake's normals of `key` at `scale`, ridges on, as the worker hands them to the atlas. */
function bake(key: PatchKey, scale: Scale): NormalsTile {
  const patch = bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    VertexPath.FaceDifferences,
    scale.scale,
    Ridges.On,
    0,
  );
  try {
    return { pairs: packNormals(patch.normals()), samplesPerSide: scale.samples };
  } finally {
    patch.free();
  }
}

/** The angle between unit `a` and unit `b`, radians, from their dot product (small angles kept). */
function angleRad(a: Vec3, b: Vec3): number {
  const d = Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z);
  return 2 * Math.asin(Math.min(d / 2, 1));
}

/**
 * A coarse–fine edge: `fine`, one of its parent's children on the parent's edge, and `coarse`, the
 * parent's neighbour across that edge at the parent's level, with where the edge lies in each.
 */
interface Edge {
  readonly name: string;
  readonly fine: PatchKey;
  readonly coarse: PatchKey;
  /** The fine patch's place (u, v) at t ∈ [0, 1] along the edge. */
  readonly fineAt: (t: number) => readonly [number, number];
  /** The coarse neighbour's place at the same point. */
  readonly coarseAt: (t: number) => readonly [number, number];
}

/** Level 11 of the test planet, ridges on: about 4.9 km patches with relief at every level. */
const PARENT: PatchKey = { face: 1, level: 11, i: 1_030, j: 1_217 };

const EDGES: ReadonlyArray<Edge> = [
  {
    // The parent's east edge, its child of quadrant (1, 0) along it, the neighbour i + 1.
    name: "east",
    fine: { ...PARENT, level: 12, i: 2 * PARENT.i + 1, j: 2 * PARENT.j },
    coarse: { ...PARENT, i: PARENT.i + 1 },
    fineAt: (t) => [1, t],
    coarseAt: (t) => [0, t / 2],
  },
  {
    // The parent's north edge, its child of quadrant (1, 1) along it, the neighbour j + 1.
    name: "north",
    fine: { ...PARENT, level: 12, i: 2 * PARENT.i + 1, j: 2 * PARENT.j + 1 },
    coarse: { ...PARENT, j: PARENT.j + 1 },
    fineAt: (t) => [t, 1],
    coarseAt: (t) => [(1 + t) / 2, 0],
  },
];

/** Points along an edge: every sample of the fine tile's edge and the midpoints between them. */
function along(samples: number): number[] {
  const steps = 2 * (samples - 1);
  return Array.from({ length: steps + 1 }, (_, n) => n / steps);
}

describe("the shading normal's morph at a coarse–fine edge (R10.T8.c)", () => {
  const bakeOnce = memoised(
    ({ key, scale }: { key: PatchKey; scale: Scale }) => bake(key, scale),
    ({ key, scale }) => `${patchKeyString(key)} ${scale.name}`,
  );

  beforeAll(() => {
    initSync({ module: wasmBytes() });
  });

  for (const scale of SCALES) {
    for (const edge of EDGES) {
      const tiles = (): { fine: NormalsTile; parent: NormalsTile; coarse: NormalsTile } => {
        const parent = parentKey(edge.fine);
        if (parent === null) {
          throw new Error("the fine patch has no parent");
        }
        return {
          fine: bakeOnce({ key: edge.fine, scale }),
          parent: bakeOnce({ key: parent, scale }),
          coarse: bakeOnce({ key: edge.coarse, scale }),
        };
      };

      it(`shades a fine patch at k = 1 as its coarse neighbour along the ${edge.name} edge (${scale.name})`, () => {
        const { fine, parent, coarse } = tiles();
        const quadrant = parentQuadrant(edge.fine);
        let worst = 0;
        for (const t of along(scale.samples)) {
          const [u, v] = edge.fineAt(t);
          const [cu, cv] = edge.coarseAt(t);
          const morphed = shadingNormal(fine, parent, quadrant, u, v, 1);
          // The coarse neighbour at k = 0 draws its own tile.
          const neighbour = shadingNormal(coarse, null, 0, cu, cv, 0);
          worst = Math.max(worst, angleRad(morphed, neighbour));
        }
        // Both read the same samples of the parent's level along the edge, bit for bit (the bakes'
        // at one direction and level), with the same weights: about 10⁻¹⁶ rad here.
        expect(worst).toBeLessThan(1e-12);
      });

      it(`shades a fine patch at k = 1 as its parent's tile at its place (${edge.name}, ${scale.name})`, () => {
        const { fine, parent } = tiles();
        const quadrant = parentQuadrant(edge.fine);
        for (const t of along(scale.samples)) {
          const [u, v] = edge.fineAt(t);
          const [pu, pv] = [((quadrant & 1) + u) / 2, (((quadrant >> 1) & 1) + v) / 2];
          expect(
            angleRad(shadingNormal(fine, parent, quadrant, u, v, 1), tileNormalAt(parent, pu, pv)),
          ).toBeLessThan(1e-12);
        }
      });

      it(`differs from its neighbour at k = 0 along the ${edge.name} edge, as its own tile (${scale.name})`, () => {
        const { fine, parent, coarse } = tiles();
        const quadrant = parentQuadrant(edge.fine);
        let worst = 0;
        for (const t of along(scale.samples)) {
          const [u, v] = edge.fineAt(t);
          const [cu, cv] = edge.coarseAt(t);
          const own = shadingNormal(fine, parent, quadrant, u, v, 0);
          expect(angleRad(own, tileNormalAt(fine, u, v))).toBe(0);
          worst = Math.max(worst, angleRad(own, tileNormalAt(coarse, cu, cv)));
        }
        // The finer level's relief tilts the normal along the edge, by 4° to 5° at worst on these
        // bakes: the step the morph removes, which a sun a few degrees up shows.
        expect(worst).toBeGreaterThan(0.02);
      });
    }
  }

  /** The low setting's tiles of the first edge's fine patch and of its parent. */
  const firstPair = (): { fine: NormalsTile; parent: NormalsTile; quadrant: number } => {
    const scale = SCALES[0];
    const edge = EDGES[0];
    if (edge === undefined) {
      throw new Error("no edge");
    }
    const parentOf = parentKey(edge.fine);
    if (parentOf === null) {
      throw new Error("the fine patch has no parent");
    }
    return {
      fine: bakeOnce({ key: edge.fine, scale }),
      parent: bakeOnce({ key: parentOf, scale }),
      quadrant: parentQuadrant(edge.fine),
    };
  };

  it("is the own tile's at every k where no parent is resident", () => {
    const { fine, quadrant } = firstPair();
    const [u, v] = [0.37, 0.61];
    for (const k of [0, 0.25, 0.5, 1]) {
      expect(angleRad(shadingNormal(fine, null, quadrant, u, v, k), tileNormalAt(fine, u, v))).toBe(
        0,
      );
    }
  });

  it("is continuous in k, and of unit length", () => {
    const { fine, parent, quadrant } = firstPair();
    const [u, v] = [0.37, 0.61];
    let previous = shadingNormal(fine, parent, quadrant, u, v, 0);
    const span = angleRad(previous, shadingNormal(fine, parent, quadrant, u, v, 1));
    for (let n = 1; n <= 100; n += 1) {
      const next = shadingNormal(fine, parent, quadrant, u, v, n / 100);
      expect(angleRad(previous, next)).toBeLessThanOrEqual(span / 50);
      expect(dot(next, next)).toBeCloseTo(1, 12);
      previous = next;
    }
  });
});
