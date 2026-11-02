/**
 * Terrain as the cluster path's first consumer (plan R14, R14.T1.b): a patch's 96 clusters, its
 * records written when its slot is filled, and the TypeScript twin of the vertex stage's decode
 * (`terrainCluster.wgsl`).
 *
 * @remarks
 * A patch of the 65-grid is 64 blocks of 8 × 8 quads, cluster 8 by + bx for the block whose quads
 * run from (8 bx, 8 by), then 32 skirt-foot strips, cluster 64 + 8 e + m for the m-th strip of edge
 * e, anticlockwise from the edge's first vertex (`gpu/resources.ts`' edge numbering: 0 y = 0,
 * 1 x = 64, 2 y = 64, 3 x = 0).
 *
 * **Bounds.** A block's footprint is exactly that of the patch three levels finer, (f, L + 3,
 * 8i + bx, 8j + by), so its sphere is that descendant's `patchBounds` at the bake's height range:
 * conservative by R05's Design note 8, over every point of the block's surface at every height in
 * the range, and so over its vertices, their morph targets (a mean of two of them) and every mix
 * between, the sphere being convex (plan R14's design note on the cluster format). A strip's is
 * the block its feet hang from, its range's floor lowered to min(h_min − d, 0), d the slot's skirt
 * depth, where the feet hang, and by h_max − h_min more: a foot hangs along its own vertex's normal
 * from its morphed place, a mean of its neighbours' at their heights, which a foot of theirs at a
 * height that much lower bounds. Each radius is then padded by {@link CLUSTER_PAD_M} and
 * {@link CLUSTER_PAD_FRACTION} of the sphere's reach from the origin, for the `f32` arithmetic that
 * places the vertices (R05's 1 mm bound on `FaceDifferences`, a few ulps of `BakedOffsets`'
 * narrowed offsets) and the record's own narrowing, and a strip's by {@link STRIP_NORMAL_FRACTION}
 * of its height span more, for the foot's normal against its neighbours' (second order in a quad's
 * angle, at most 1/64 rad at level 0). `clusters/terrain.test.ts` holds every vertex of random
 * patches inside its clusters' spheres.
 */

import { type PatchKey } from "../terrain/patchKey";
import { patchBounds } from "../terrain/bounds";
import type { PlanetGeometry } from "../terrain/planet";
import type { Vec3 } from "../../geometry/vec3";
import { CLUSTER_RECORD_BYTES, type ClusterShape, packClusterRecord } from "./record";
import {
  CLUSTER_PAD_FRACTION,
  CLUSTER_PAD_M,
  CLUSTER_QUADS,
  edgeVertex,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
} from "./terrainGrid";

export {
  CLUSTER_PAD_FRACTION,
  CLUSTER_PAD_M,
  edgeVertex,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
} from "./terrainGrid";

/** Quads along a patch's side. */
const PATCH_QUADS = 64;

/** Blocks along a patch's side, and strips along an edge. */
const BLOCKS_PER_SIDE = PATCH_QUADS / CLUSTER_QUADS;

/** A strip's further padding per metre of its height span (see the module's remarks). */
export const STRIP_NORMAL_FRACTION = 1e-3;

/** Where cluster `cluster` of a terrain patch lies (see the module's remarks). */
export interface TerrainClusterPlace {
  readonly shape: Extract<ClusterShape, "terrain-block" | "terrain-skirt">;
  /** A block's column and row of blocks, or a strip's block along its edge, m, in bx. */
  readonly bx: number;
  readonly by: number;
  /** A strip's edge, 0 to 3; 0 for a block. */
  readonly edge: number;
  /** A strip's place along its edge, 0 to 7; 0 for a block. */
  readonly strip: number;
}

/** Cluster `cluster`'s place, 0 to {@link TERRAIN_CLUSTERS_PER_PAGE} − 1. */
export function terrainClusterPlace(cluster: number): TerrainClusterPlace {
  if (!Number.isInteger(cluster) || cluster < 0 || cluster >= TERRAIN_CLUSTERS_PER_PAGE) {
    throw new RangeError(`a terrain patch has no cluster ${String(cluster)}`);
  }
  if (cluster < TERRAIN_BLOCKS) {
    return {
      shape: "terrain-block",
      bx: cluster % BLOCKS_PER_SIDE,
      by: Math.floor(cluster / BLOCKS_PER_SIDE),
      edge: 0,
      strip: 0,
    };
  }
  const edge = Math.floor((cluster - TERRAIN_BLOCKS) / BLOCKS_PER_SIDE);
  const strip = (cluster - TERRAIN_BLOCKS) % BLOCKS_PER_SIDE;
  const [bx, by] = stripBlock(edge, strip);
  return { shape: "terrain-skirt", bx, by, edge, strip };
}

/** The block a strip's feet hang from: its edge's quads 8 m to 8 m + 8 lie along that block's side. */
function stripBlock(edge: number, strip: number): readonly [number, number] {
  const last = BLOCKS_PER_SIDE - 1;
  switch (edge) {
    case 0:
      return [strip, 0];
    case 1:
      return [last, strip];
    case 2:
      return [last - strip, last];
    default:
      return [0, last - strip];
  }
}

/**
 * The record's template reference of `cluster` (`clusters/record.ts`' remarks): its first vertex's
 * column and row, and a strip's edge.
 */
export function terrainTemplateRef(cluster: number): number {
  const place = terrainClusterPlace(cluster);
  const [x0, y0] =
    place.shape === "terrain-block"
      ? [CLUSTER_QUADS * place.bx, CLUSTER_QUADS * place.by]
      : edgeVertex(place.edge, CLUSTER_QUADS * place.strip);
  return x0 | (y0 << 8) | (place.edge << 16);
}

/**
 * The lit mesh's grid coordinates and code of a template vertex `local` ((lx, ly, code), the
 * terrain cluster mesh's `position`) of cluster `cluster`: the vertex stage's decode, which hands
 * `lit_place` exactly the lit mesh's own vertex (`terrainCluster.wgsl`'s `terrain_cluster_grid`).
 */
export function terrainClusterGrid(
  cluster: number,
  local: readonly [number, number, number],
): readonly [number, number, number] {
  const place = terrainClusterPlace(cluster);
  const [lx, ly, code] = local;
  if (place.shape === "terrain-block") {
    return [CLUSTER_QUADS * place.bx + lx, CLUSTER_QUADS * place.by + ly, code];
  }
  const [x, y] = edgeVertex(place.edge, CLUSTER_QUADS * place.strip + lx);
  return [x, y, code];
}

/** What a terrain patch's records are made from: its bake's key, origin, heights and skirts. */
export interface TerrainClusterSource {
  readonly key: PatchKey;
  /** The bake's origin, body-fixed, m (`f64`): the page's origin. */
  readonly originM: Vec3;
  /** The bake's height range, both channels', m. */
  readonly heightRangeM: readonly [number, number];
  /** The slot's skirt depth, m. */
  readonly skirtDepthM: number;
  /**
   * The bake's measured errors, four half floats a cluster in cluster order (`BakedPatch`'s
   * `clusterErrors`, plan R14, R14.T3.b): written into each record, marked measured; absent, the
   * records hold (0, +∞), unmeasured.
   */
  readonly clusterErrors?: Float16Array;
}

/**
 * Writes a terrain patch's {@link TERRAIN_CLUSTERS_PER_PAGE} records into `out` from
 * `offsetBytes`, cluster by cluster (see the module's remarks): each sphere about the bake's
 * origin, in the body-fixed axes, strided, never discarding, with the bake's measured errors where
 * it has them.
 *
 * @throws RangeError for measured errors that are not four a cluster.
 */
export function writeTerrainClusterRecords(
  planet: PlanetGeometry,
  source: TerrainClusterSource,
  out: DataView,
  offsetBytes: number,
): void {
  const { key, originM, heightRangeM, skirtDepthM, clusterErrors } = source;
  if (clusterErrors !== undefined && clusterErrors.length !== 4 * TERRAIN_CLUSTERS_PER_PAGE) {
    throw new RangeError(
      `a patch's measured errors are ${String(4 * TERRAIN_CLUSTERS_PER_PAGE)} half floats, not ${String(clusterErrors.length)}`,
    );
  }
  const errorBits =
    clusterErrors === undefined
      ? null
      : new Uint16Array(clusterErrors.buffer, clusterErrors.byteOffset, clusterErrors.length);
  const [low, high] = heightRangeM;
  const floor = Math.min(low - skirtDepthM, 0) - (high - low);
  const blocks: Array<{ readonly centreM: Vec3; readonly radiusM: number }> = [];
  for (let n = 0; n < TERRAIN_BLOCKS; n += 1) {
    blocks.push(
      blockSphere(
        planet,
        key,
        n % BLOCKS_PER_SIDE,
        Math.floor(n / BLOCKS_PER_SIDE),
        originM,
        low,
        high,
      ),
    );
  }
  for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
    const place = terrainClusterPlace(cluster);
    let sphere: { readonly centreM: Vec3; readonly radiusM: number };
    if (place.shape === "terrain-block") {
      sphere = blocks[cluster] ?? { centreM: { x: 0, y: 0, z: 0 }, radiusM: Infinity };
    } else {
      const lowered = blockSphere(planet, key, place.bx, place.by, originM, floor, high);
      sphere = {
        centreM: lowered.centreM,
        radiusM: lowered.radiusM + STRIP_NORMAL_FRACTION * (high - floor),
      };
    }
    const at = 4 * cluster;
    packClusterRecord(out, offsetBytes + cluster * CLUSTER_RECORD_BYTES, {
      centreM: sphere.centreM,
      radiusM: sphere.radiusM,
      mode: "strided",
      strideErrorBits:
        errorBits === null
          ? null
          : [
              errorBits[at] ?? 0,
              errorBits[at + 1] ?? 0,
              errorBits[at + 2] ?? 0,
              errorBits[at + 3] ?? 0,
            ],
      templateRef: terrainTemplateRef(cluster),
      shape: place.shape,
      mayDiscard: false,
      material: 0,
    });
  }
}

/** The padded sphere of block (`bx`, `by`) of `key` at heights `low` to `high`, about `originM`. */
function blockSphere(
  planet: PlanetGeometry,
  key: PatchKey,
  bx: number,
  by: number,
  originM: Vec3,
  low: number,
  high: number,
): { readonly centreM: Vec3; readonly radiusM: number } {
  const bounds = patchBounds(
    planet,
    {
      face: key.face,
      level: key.level + 3,
      i: BLOCKS_PER_SIDE * key.i + bx,
      j: BLOCKS_PER_SIDE * key.j + by,
    },
    [low, high],
  );
  const centreM = {
    x: bounds.centre.x - originM.x,
    y: bounds.centre.y - originM.y,
    z: bounds.centre.z - originM.z,
  };
  const reachM = Math.hypot(centreM.x, centreM.y, centreM.z) + bounds.radiusM;
  return { centreM, radiusM: bounds.radiusM + CLUSTER_PAD_M + CLUSTER_PAD_FRACTION * reachM };
}
