/**
 * The hulls' occluder faces as the cluster path's first DAG-cut consumer (plan R14, R14.T4.b): each
 * drawn hull a page of its opaque faces, 128 triangles a cluster, one group with no parent, so
 * that the cut draws every cluster; its records, their LOD spheres and the hulls' instances, as the
 * cull (`clusterCull.wgsl`) and `occluderCluster.wgsl` read them.
 *
 * @remarks
 * - **Pages**: a hull's faces in the order the draw list holds them (`OccluderMesh.triangles`), its
 *   cluster c their triangles 128 c to 128 c + 127, up to {@link HULL_CLUSTERS_PER_PAGE} clusters;
 *   the page's other records hold no cluster (a negative radius), which every pass refuses. A hull
 *   with more faces than a page holds is drawn by the instanced draw, as every hull is on the way
 *   back.
 * - **Records**: a cluster's sphere about the middle of its corners' box, m from the hull's
 *   reference point along the camera frame's axes, its radius the farthest corner in `f64`, padded
 *   by {@link CLUSTER_PAD_M} and {@link CLUSTER_PAD_FRACTION} of its reach, as terrain's are; the
 *   DAG-cut mode with (own, parent) errors (0, +∞), a one-level DAG whose cut is its leaves at any
 *   tolerance; its template reference its first triangle in the page and its count
 *   (`meshTemplateRef`); never discarding, material 0. Its LOD sphere, its parent group's, is its
 *   own, never projected against an error of +∞.
 * - **Instances**: {@link HULL_INSTANCE_BYTES} a hull, its reference point from the camera, `f32`
 *   as the instanced draw's `offsetFromCameraM` holds it, so that each corner is placed to the same
 *   bits, its page, and its first triangle in the frame's `corners` and its triangle count.
 */

import {
  type ClusterLod,
  CLUSTER_LOD_BYTES,
  CLUSTER_RECORD_BYTES,
  MESH_CLUSTER_TRIANGLES,
  meshTemplateRef,
  packClusterLod,
  packClusterRecord,
} from "../clusters/record";
import { CLUSTER_PAD_FRACTION, CLUSTER_PAD_M } from "../clusters/terrainGrid";
import type { OccluderMesh } from "./drawList";

/** The most clusters a hull's page holds: 1,024 faces, far past `TEST_HULL`'s 14. */
export const HULL_CLUSTERS_PER_PAGE = 8;

/** Bytes of a hull's instance record, as `occluderCluster.wgsl`'s `HullInstance` lays it out. */
export const HULL_INSTANCE_BYTES = 32;

/** A frame's hulls as the cluster path draws them (see the module's remarks). */
export interface HullClusterFrame {
  /** The hulls drawn: the list's meshes with any face, in its order. */
  readonly hulls: number;
  /** Their pages' records, {@link HULL_CLUSTERS_PER_PAGE} a page, and each record's LOD sphere. */
  readonly records: ArrayBuffer;
  readonly lods: ArrayBuffer;
  /** Their instances, {@link HULL_INSTANCE_BYTES} each. */
  readonly instances: ArrayBuffer;
  /** Their clusters and triangles in all. */
  readonly clusters: number;
  readonly triangles: number;
}

/**
 * The cluster path's frame of `meshes` (see the module's remarks), or `null` where a hull has more
 * faces than a page holds.
 */
export function hullClusterFrame(meshes: ReadonlyArray<OccluderMesh>): HullClusterFrame | null {
  const drawn = meshes.filter((mesh) => mesh.triangles.length >= 9);
  if (
    drawn.some(
      (mesh) => mesh.triangles.length / 9 > HULL_CLUSTERS_PER_PAGE * MESH_CLUSTER_TRIANGLES,
    )
  ) {
    return null;
  }
  const records = new ArrayBuffer(
    Math.max(1, drawn.length * HULL_CLUSTERS_PER_PAGE) * CLUSTER_RECORD_BYTES,
  );
  const lods = new ArrayBuffer(
    Math.max(1, drawn.length * HULL_CLUSTERS_PER_PAGE) * CLUSTER_LOD_BYTES,
  );
  const instances = new ArrayBuffer(Math.max(1, drawn.length) * HULL_INSTANCE_BYTES);
  const recordView = new DataView(records);
  const lodView = new DataView(lods);
  const instanceView = new DataView(instances);
  let firstTriangle = 0;
  let clusters = 0;
  drawn.forEach((mesh, page) => {
    const triangles = mesh.triangles.length / 9;
    const at = page * HULL_INSTANCE_BYTES;
    instanceView.setFloat32(at, mesh.originF32[0] ?? 0, true);
    instanceView.setFloat32(at + 4, mesh.originF32[1] ?? 0, true);
    instanceView.setFloat32(at + 8, mesh.originF32[2] ?? 0, true);
    instanceView.setUint32(at + 12, page, true);
    instanceView.setUint32(at + 16, firstTriangle, true);
    instanceView.setUint32(at + 20, triangles, true);
    for (let c = 0; c < HULL_CLUSTERS_PER_PAGE; c += 1) {
      const record = (page * HULL_CLUSTERS_PER_PAGE + c) * CLUSTER_RECORD_BYTES;
      const lod = (page * HULL_CLUSTERS_PER_PAGE + c) * CLUSTER_LOD_BYTES;
      const first = c * MESH_CLUSTER_TRIANGLES;
      const count = Math.min(MESH_CLUSTER_TRIANGLES, triangles - first);
      if (count <= 0) {
        // No cluster: a negative radius, which every pass refuses.
        recordView.setFloat32(record + 12, -1, true);
        continue;
      }
      const sphere = cornersSphere(mesh.triangles, first, count);
      packClusterRecord(recordView, record, {
        centreM: sphere.parentCentreM,
        radiusM: sphere.parentRadiusM,
        mode: "dag",
        errorsM: [0, Infinity],
        templateRef: meshTemplateRef(first, count),
        shape: "mesh",
        mayDiscard: false,
        material: 0,
      });
      packClusterLod(lodView, lod, sphere);
      clusters += 1;
    }
    firstTriangle += triangles;
  });
  return { hulls: drawn.length, records, lods, instances, clusters, triangles: firstTriangle };
}

/**
 * The padded sphere of `count` triangles of `corners` (nine `f32` a triangle) from triangle
 * `first`: about the middle of their corners' box, its radius the farthest corner (see the
 * module's remarks).
 */
function cornersSphere(corners: Float32Array, first: number, count: number): ClusterLod {
  const low = [Infinity, Infinity, Infinity];
  const high = [-Infinity, -Infinity, -Infinity];
  for (let n = 9 * first; n < 9 * (first + count); n += 3) {
    for (let k = 0; k < 3; k += 1) {
      const v = corners[n + k] ?? Number.NaN;
      low[k] = Math.min(low[k] ?? Infinity, v);
      high[k] = Math.max(high[k] ?? -Infinity, v);
    }
  }
  const [cx, cy, cz] = [0, 1, 2].map((k) => ((low[k] ?? 0) + (high[k] ?? 0)) / 2);
  let radiusM = 0;
  for (let n = 9 * first; n < 9 * (first + count); n += 3) {
    radiusM = Math.max(
      radiusM,
      Math.hypot(
        (corners[n] ?? Number.NaN) - (cx ?? 0),
        (corners[n + 1] ?? Number.NaN) - (cy ?? 0),
        (corners[n + 2] ?? Number.NaN) - (cz ?? 0),
      ),
    );
  }
  const reachM = Math.hypot(cx ?? 0, cy ?? 0, cz ?? 0) + radiusM;
  return {
    parentCentreM: { x: cx ?? 0, y: cy ?? 0, z: cz ?? 0 },
    parentRadiusM: radiusM + CLUSTER_PAD_M + CLUSTER_PAD_FRACTION * reachM,
  };
}
