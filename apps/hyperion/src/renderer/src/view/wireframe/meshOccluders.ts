/**
 * The DAG consumers' occluder depth in the wireframe (plan R14, R14.T5.f, Design note 13): meshes
 * of a mesh page pool (`clusters/pages.ts`) drawn depth only through the cluster path's DAG cut at
 * `meshTolerancePx.wireframeOccluder`, under the strokes' half-width, so that a coarser cut's
 * silhouette moves less than any stroke's half-width and hidden-line removal holds.
 *
 * @remarks
 * - **Instances**: one a resident page of each mesh drawn, {@link MESH_OCCLUDER_INSTANCE_BYTES}:
 *   its pages' origin from the camera along the camera frame's axes, the mesh's origin from the
 *   camera plus its pages' origin in its frame, narrowed once to `f32` (the cull's first three
 *   words); its slot, the cull's page; and its page's first word in the pool's pages, which
 *   `meshOccluderCluster.wgsl` decodes. A mesh's frame is the camera frame's axes: an instance's
 *   rotation is T5.d's (lean, 2026-11-02).
 * - **Records and LOD spheres** are the pool's, as its record rewrites leave them, 128 a slot: a
 *   missing page's resident coarser group draws in its place (T5.b).
 * - **The tolerance** is {@link wireframeToleranceRad}'s: the setting's px over the view's px per
 *   radian at its corners, the most anywhere in it, as `CameraCullPass.toleranceRad` takes it.
 * - **Depth**: each corner is pushed away from the camera along its ray by its cluster's own error
 *   as built, so that a coarser cut's faces lie no nearer than its leaves' and hide no edge the
 *   finest cut shows; the occluders' slope term covers a face seen edge on, whose error is under
 *   the tolerance across the view (lean, 2026-11-02).
 */

import type { Viewport } from "../camera/projection";
import { MAX_PAGE_CLUSTERS } from "../clusters/pageFormat";
import { type GeometryPagePool, pageSlotOffsetBytes } from "../clusters/pages";

/** A mesh of the wireframe's page pool drawn as an occluder. */
export interface MeshOccluder {
  /** The mesh's stable name, the same from frame to frame. */
  readonly id: string;
  /** Its mesh in the pool (`GeometryPagePool.addMesh`'s handle). */
  readonly mesh: number;
  /** Its frame's origin from the camera, along the camera frame's axes, `f32` m. */
  readonly originF32: Float32Array;
}

/** Bytes of a mesh occluder's instance, as `meshOccluderCluster.wgsl`'s `MeshInstance` lays it. */
export const MESH_OCCLUDER_INSTANCE_BYTES = 32;

/** The clusters a mesh occluder's page holds: the pool's records a slot. */
export const MESH_OCCLUDER_CLUSTERS_PER_PAGE = MAX_PAGE_CLUSTERS;

/** A frame's mesh occluders as the cluster path culls and draws them (see the module's remarks). */
export interface MeshOccluderFrame {
  /** Their instances, a resident page each, {@link MESH_OCCLUDER_INSTANCE_BYTES} each. */
  readonly instances: ArrayBuffer;
  readonly count: number;
  /** The meshes with a resident page, of those asked. */
  readonly meshes: number;
}

/**
 * The cluster path's frame of `occluders` over `pool` (see the module's remarks), or `null` where
 * none has a resident page.
 */
export function meshOccluderFrame(
  pool: Pick<GeometryPagePool, "residentPages" | "originOf" | "layout">,
  occluders: ReadonlyArray<MeshOccluder>,
): MeshOccluderFrame | null {
  const pages = occluders.flatMap((occluder) => {
    const originM = pool.originOf(occluder.mesh);
    return originM === undefined
      ? []
      : pool.residentPages(occluder.mesh).map(([, slot]) => ({ occluder, originM, slot }));
  });
  if (pages.length === 0) {
    return null;
  }
  const instances = new ArrayBuffer(pages.length * MESH_OCCLUDER_INSTANCE_BYTES);
  const view = new DataView(instances);
  pages.forEach(({ occluder, originM, slot }, n) => {
    const at = n * MESH_OCCLUDER_INSTANCE_BYTES;
    [originM.x, originM.y, originM.z].forEach((pageM, k) => {
      view.setFloat32(at + 4 * k, (occluder.originF32[k] ?? 0) + pageM, true);
    });
    view.setUint32(at + 12, slot, true);
    view.setUint32(at + 16, pageSlotOffsetBytes(pool.layout, slot) / 4, true);
  });
  const meshes = new Set(pages.map(({ occluder }) => occluder.mesh)).size;
  return { instances, count: pages.length, meshes };
}

/**
 * A tolerance of `tolerancePx` as the camera pass takes it, rad: over the view's px per radian at
 * its corners, f (1 + tan² θ_c), f = W ÷ (2 tan(fov_h ÷ 2)) and θ_c the corner's angle off the
 * axis, so that an angle of it is at most `tolerancePx` anywhere in the view.
 */
export function wireframeToleranceRad(
  tolerancePx: number,
  fovXRad: number,
  viewport: Viewport,
): number {
  const tanHalf = Math.tan(fovXRad / 2);
  const focalPx = viewport.widthPx / (2 * tanHalf);
  const aspect = viewport.heightPx / viewport.widthPx;
  return tolerancePx / (focalPx * (1 + tanHalf * tanHalf * (1 + aspect * aspect)));
}
