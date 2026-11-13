/**
 * A brute-force oracle of a terrain cluster's stride error (plan R14, R14.T3.a and T3.b's test),
 * shared by `clusters/errors.test.ts` and `clusters/errors.wasm.test.ts`: the distance between a
 * cluster drawn at stride s and at stride 1, measured at stride-1 points through the cluster mesh's
 * own templates and the GPU's own vertex placement, with nothing of `clusters/errors.ts`.
 *
 * @remarks
 * A vertex is placed as `lit_place` places it, at one morph factor k for the whole cluster: a grid
 * vertex at mix(q₀, q₁, k), a skirt foot below it along ν to the height min(h − d, 0). On
 * `face-differences` q₀, q₁ and ν are `vertexEmulation.ts`' `f32` arithmetic, the shader's own; on
 * `baked-offsets` q₀ and q₁ are the bake's offsets. Each stride-1 point is found in the stride-s
 * template's triangles by its place in the cluster, (lx, ly) for a block and (k, 1 − code) for a
 * strip, and the stride-s mesh is taken there by the barycentric mix of that triangle's vertices.
 */

import type { Vec3 } from "../geometry/vec3";
import { terrainClusterGrid, TERRAIN_BLOCKS } from "../view/clusters/terrain";
import { type Stride, STRIDES, terrainClusterMesh } from "../view/clusters/templates";
import { patchTerms } from "../view/terrain/gpu/uniforms";
import {
  directionDifferenceF32,
  faceDifferenceMorphF32,
  faceDifferencePositionF32,
  normalDifferenceF32,
  type SlotTermsF32,
} from "../view/terrain/gpu/vertexEmulation";
import type { PatchKey } from "../view/terrain/patchKey";
import type { BodyFigure } from "../view/terrain/planet";
import { PATCH_VERTICES_PER_SIDE } from "../view/terrain/slotLayout";

const MESH = terrainClusterMesh();

/** Vertices along a patch's side. */
const SIDE = PATCH_VERTICES_PER_SIDE;

/** What the oracle places a patch from: a bake's heights, offsets and skirt depth, and its key. */
export interface OracleBake {
  readonly key: PatchKey;
  readonly figure: BodyFigure;
  /** Own and morph heights per vertex, interleaved, metres. */
  readonly heights: Float32Array;
  /** q₀ then q₁ per vertex, metres, on `baked-offsets`; `null` on `face-differences`. */
  readonly offsets: Float32Array | null;
  readonly originHeightM: number;
  readonly skirtDepthM: number;
}

/** `key`'s slot terms on `figure` at origin height `h0M`, narrowed as its slot record holds them. */
export function slotTermsF32(key: PatchKey, figure: BodyFigure, h0M: number): SlotTermsF32 {
  const terms = patchTerms(key, figure, h0M);
  const f = Math.fround;
  const v = (a: Vec3): Vec3 => ({ x: f(a.x), y: f(a.y), z: f(a.z) });
  const [a, e1, e2] = terms.axes;
  return {
    axisA: v(a),
    axisE1: v(e1),
    axisE2: v(e2),
    s0: f(terms.st0[0]),
    t0: f(terms.st0[1]),
    u0: f(terms.uv0[0]),
    v0: f(terms.uv0[1]),
    step: f(terms.step),
    scale: v(terms.scale),
    m0: v(terms.m0),
    nu0: v(terms.nu0),
    h0M: f(h0M),
    straddles: terms.straddles,
  };
}

/** A patch placed as the GPU places it: each vertex's (x, y, code) at morph factor k. */
export type PlaceVertex = (x: number, y: number, code: number, k: number) => Vec3;

/** The GPU's placement of `bake`'s vertices (see the module's remarks). */
export function gpuPlacement(bake: OracleBake): PlaceVertex {
  const rec = slotTermsF32(bake.key, bake.figure, bake.originHeightM);
  const own = (x: number, y: number): number => bake.heights[2 * (y * SIDE + x)] ?? Number.NaN;
  const morph = (x: number, y: number): number =>
    bake.heights[2 * (y * SIDE + x) + 1] ?? Number.NaN;
  const offsets = bake.offsets;
  const at = (x: number, y: number, which: 0 | 1): Vec3 => {
    if (offsets !== null) {
      const i = 6 * (y * SIDE + x) + 3 * which;
      return {
        x: offsets[i] ?? Number.NaN,
        y: offsets[i + 1] ?? Number.NaN,
        z: offsets[i + 2] ?? Number.NaN,
      };
    }
    return which === 0
      ? faceDifferencePositionF32(rec, x, y, own(x, y))
      : faceDifferenceMorphF32(rec, x, y, morph);
  };
  return (x, y, code, k) => {
    const q0 = at(x, y, 0);
    const q1 = at(x, y, 1);
    const q = {
      x: q0.x + k * (q1.x - q0.x),
      y: q0.y + k * (q1.y - q0.y),
      z: q0.z + k * (q1.z - q0.z),
    };
    if (code === 0) {
      return q;
    }
    const h = own(x, y) * (1 - k) + morph(x, y) * k;
    const drop = Math.min(h - bake.skirtDepthM, 0) - h;
    const d = normalDifferenceF32(rec, directionDifferenceF32(rec, x, y));
    const nu = { x: rec.nu0.x + d.x, y: rec.nu0.y + d.y, z: rec.nu0.z + d.z };
    return { x: q.x + drop * nu.x, y: q.y + drop * nu.y, z: q.z + drop * nu.z };
  };
}

/** One corner of a template triangle: its place in the cluster and its drawn point. */
interface Corner {
  readonly u: number;
  readonly v: number;
  readonly p: Vec3;
}

/** Cluster `cluster`'s triangles at `stride`, placed at morph factor `k`. */
function triangles(
  place: PlaceVertex,
  cluster: number,
  stride: Stride,
  k: number,
): ReadonlyArray<readonly [Corner, Corner, Corner]> {
  const shape = cluster < TERRAIN_BLOCKS ? 0 : 1;
  const template = MESH.templates[shape * STRIDES.length + STRIDES.indexOf(stride)];
  if (template === undefined) {
    throw new Error(`no template of stride ${String(stride)}`);
  }
  const indices = MESH.mesh.indices ?? new Uint32Array();
  const positions = MESH.mesh.positions;
  const corner = (n: number): Corner => {
    const i = indices[template.firstIndex + n] ?? 0;
    const local = [
      positions[3 * i] ?? 0,
      positions[3 * i + 1] ?? 0,
      positions[3 * i + 2] ?? 0,
    ] as const;
    const [x, y, code] = terrainClusterGrid(cluster, local);
    return shape === 0
      ? { u: local[0], v: local[1], p: place(x, y, code, k) }
      : { u: local[0], v: 1 - local[2], p: place(x, y, code, k) };
  };
  const out: Array<readonly [Corner, Corner, Corner]> = [];
  for (let n = 0; n < template.indexCount; n += 3) {
    out.push([corner(n), corner(n + 1), corner(n + 2)]);
  }
  return out;
}

/** The barycentric weights of (u, v) in `t`, or `null` where it lies outside. */
function weightsIn(
  t: readonly [Corner, Corner, Corner],
  u: number,
  v: number,
): readonly [number, number, number] | null {
  const [a, b, c] = t;
  const det = (b.u - a.u) * (c.v - a.v) - (c.u - a.u) * (b.v - a.v);
  const wb = ((u - a.u) * (c.v - a.v) - (c.u - a.u) * (v - a.v)) / det;
  const wc = ((b.u - a.u) * (v - a.v) - (u - a.u) * (b.v - a.v)) / det;
  const wa = 1 - wb - wc;
  const slack = -1e-9;
  return wa >= slack && wb >= slack && wc >= slack ? [wa, wb, wc] : null;
}

function mix(t: readonly [Corner, Corner, Corner], w: readonly [number, number, number]): Vec3 {
  const [a, b, c] = t;
  return {
    x: w[0] * a.p.x + w[1] * b.p.x + w[2] * c.p.x,
    y: w[0] * a.p.y + w[1] * b.p.y + w[2] * c.p.y,
    z: w[0] * a.p.z + w[1] * b.p.z + w[2] * c.p.z,
  };
}

/**
 * The largest distance between cluster `cluster` drawn at `stride` and at stride 1, at morph factor
 * `k`, over every stride-1 vertex and, where `lattice` is above 1, the points (i, j) ÷ `lattice` of
 * every stride-1 triangle's barycentric lattice, metres.
 *
 * @throws Error where a stride-1 point lies in no stride-s triangle (the templates do not cover
 *   the same place).
 */
export function strideDistanceM(
  place: PlaceVertex,
  cluster: number,
  stride: Stride,
  k: number,
  lattice = 1,
): number {
  const coarse = triangles(place, cluster, stride, k);
  let worst = 0;
  for (const fine of triangles(place, cluster, 1, k)) {
    for (let i = 0; i <= lattice; i += 1) {
      for (let j = 0; i + j <= lattice; j += 1) {
        const w = [i / lattice, j / lattice, (lattice - i - j) / lattice] as const;
        const u = w[0] * fine[0].u + w[1] * fine[1].u + w[2] * fine[2].u;
        const v = w[0] * fine[0].v + w[1] * fine[1].v + w[2] * fine[2].v;
        const p = mix(fine, w);
        const holder = coarse.find((t) => weightsIn(t, u, v) !== null);
        const weights = holder === undefined ? null : weightsIn(holder, u, v);
        if (holder === undefined || weights === null) {
          throw new Error(
            `cluster ${String(cluster)}'s point (${String(u)}, ${String(v)}) lies in no stride-${String(stride)} triangle`,
          );
        }
        const q = mix(holder, weights);
        worst = Math.max(worst, Math.hypot(p.x - q.x, p.y - q.y, p.z - q.z));
      }
    }
  }
  return worst;
}
