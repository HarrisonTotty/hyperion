/**
 * The terrain cluster records' bounds check (plan R14, R14.T1.b and T1.f), shared by
 * `clusters/terrain.test.ts`' random patches and `clusters/terrainBounds.wasm.test.ts`' real bakes:
 * every vertex the GPU places of a patch, at its own height and its morph target and between, and
 * every skirt foot, against the spheres of the clusters that draw it.
 */

import type { Vec3 } from "../geometry/vec3";
import { CLUSTER_RECORD_BYTES } from "../view/clusters/record";
import {
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_PAGE_CLUSTERS,
  terrainClusterGrid,
  terrainFinGrid,
  writeTerrainClusterRecords,
} from "../view/clusters/terrain";
import {
  STRIDES,
  terrainClusterMesh,
  terrainDiscardTemplateIndex,
} from "../view/clusters/templates";
import { patchTerms } from "../view/terrain/gpu/uniforms";
import {
  directionDifferenceF32,
  faceDifferenceMorphF32,
  faceDifferencePositionF32,
  normalDifferenceF32,
  type SlotTermsF32,
} from "../view/terrain/gpu/vertexEmulation";
import type { PatchKey } from "../view/terrain/patchKey";
import { type BodyFigure, planetGeometry } from "../view/terrain/planet";

const MESH = terrainClusterMesh();

/** `key`'s terms on `figure` at origin height `h0M`, narrowed as its slot record holds them. */
export function termsF32(key: PatchKey, figure: BodyFigure, h0M: number): SlotTermsF32 {
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

/** One patch's bake as the bounds check takes it. */
export interface ClusterBoundsPatch {
  readonly figure: BodyFigure;
  readonly key: PatchKey;
  /** The bake's origin, body-fixed, m (`f64`), and its height there, m. */
  readonly originM: Vec3;
  readonly originHeightM: number;
  /** Each vertex's own height and its morph target, m, at 65 y + x. */
  readonly own: ArrayLike<number>;
  readonly morph: ArrayLike<number>;
  /** The bake's height range, both channels', m, and its skirts' depth, m. */
  readonly heightRangeM: readonly [number, number];
  readonly skirtDepthM: number;
  /**
   * The slot's survey mask as its field holds it (plan R14, R14.T1.g), by which the records are
   * classified and the fin clusters drawn; absent, an interior patch's.
   */
  readonly surveyMask?: Uint8Array;
}

/** A template vertex's (lx, ly, code). */
function local(v: number): readonly [number, number, number] {
  const p = MESH.mesh.positions;
  return [p[3 * v] ?? Number.NaN, p[3 * v + 1] ?? Number.NaN, p[3 * v + 2] ?? Number.NaN];
}

/**
 * The largest distance, m, by which any vertex the GPU places of `patch` lies outside the sphere of
 * a cluster that draws it at stride 1, at its own height, its morph target and three mixes between,
 * as a skirt foot (`lit_place`'s min(h − d, 0) along the datum's normal), and, of a fin cluster the
 * survey draws (plan R14, R14.T1.g), as a skirt head (max(h, 0)) or a fin's datum end (0): below 0
 * where every vertex is inside. A cluster that is none (a negative radius) is not checked.
 */
export function worstOutsideClusters(patch: ClusterBoundsPatch): number {
  const { figure, key, own, morph, heightRangeM, skirtDepthM, surveyMask } = patch;
  const records = new DataView(new ArrayBuffer(TERRAIN_PAGE_CLUSTERS * CLUSTER_RECORD_BYTES));
  writeTerrainClusterRecords(
    planetGeometry(figure, null),
    {
      key,
      originM: patch.originM,
      heightRangeM,
      skirtDepthM,
      ...(surveyMask === undefined ? {} : { surveyMask }),
    },
    records,
    0,
  );
  const rec = termsF32(key, figure, patch.originHeightM);
  let worst = -Infinity;
  const check = (cluster: number, p: Vec3): void => {
    const at = cluster * CLUSTER_RECORD_BYTES;
    const cx = records.getFloat32(at, true);
    const cy = records.getFloat32(at + 4, true);
    const cz = records.getFloat32(at + 8, true);
    const r = records.getFloat32(at + 12, true);
    worst = Math.max(worst, Math.hypot(p.x - cx, p.y - cy, p.z - cz) - r);
  };
  const indices = MESH.mesh.indices ?? new Uint32Array();
  for (let cluster = 0; cluster < TERRAIN_PAGE_CLUSTERS; cluster += 1) {
    if (records.getFloat32(cluster * CLUSTER_RECORD_BYTES + 12, true) < 0) {
      continue;
    }
    const fin = cluster >= TERRAIN_CLUSTERS_PER_PAGE;
    const template =
      MESH.templates[
        fin
          ? terrainDiscardTemplateIndex(
              cluster < TERRAIN_CLUSTERS_PER_PAGE + 8 ? "terrain-head" : "terrain-fin",
            )
          : cluster < TERRAIN_BLOCKS
            ? 0
            : STRIDES.length
      ];
    for (let n = 0; n < (template?.indexCount ?? 0); n += 1) {
      const vertex = local(indices[(template?.firstIndex ?? 0) + n] ?? 0);
      const [x, y, code] = fin
        ? terrainFinGrid(cluster, vertex)
        : terrainClusterGrid(cluster, vertex);
      const h0 = own[y * 65 + x] ?? 0;
      const h1 = morph[y * 65 + x] ?? 0;
      const q0 = faceDifferencePositionF32(rec, x, y, h0);
      const q1 = faceDifferenceMorphF32(rec, x, y, (px, py) => morph[py * 65 + px] ?? 0);
      const d = normalDifferenceF32(rec, directionDifferenceF32(rec, x, y));
      const nu = { x: rec.nu0.x + d.x, y: rec.nu0.y + d.y, z: rec.nu0.z + d.z };
      for (const k of [0, 0.25, 0.5, 0.75, 1]) {
        const q = {
          x: q0.x + k * (q1.x - q0.x),
          y: q0.y + k * (q1.y - q0.y),
          z: q0.z + k * (q1.z - q0.z),
        };
        if (code === 0) {
          check(cluster, q);
        } else {
          const h = h0 * (1 - k) + h1 * k;
          // A foot hangs to min(h − d, 0), a head rises to max(h, 0), a fin's datum end stands at 0
          // (`lit_place`).
          const to = code === 1 ? Math.min(h - skirtDepthM, 0) : code === 2 ? Math.max(h, 0) : 0;
          const drop = to - h;
          check(cluster, { x: q.x + drop * nu.x, y: q.y + drop * nu.y, z: q.z + drop * nu.z });
        }
      }
    }
  }
  return worst;
}
