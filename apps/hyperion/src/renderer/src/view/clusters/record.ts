/**
 * The cluster path's static record (plan R14, R14.T1.b; the research `research-terrain-gpu-driven`,
 * §5): one a cluster of at most 128 triangles, written once when its page is filled, which every
 * pass's cull reads (`clusterCull.wgsl`).
 *
 * @remarks
 * A record holds the cluster's bounds about its page's origin, in the page's own axes (terrain: a
 * patch's, body-fixed), its error bytes, its template reference and its flags, 32 B, as WGSL's
 * `ClusterRecord` (`clusters.wgsl`) lays it out:
 *
 * | Bytes | Field          | Meaning                                                                |
 * | ----- | -------------- | ---------------------------------------------------------------------- |
 * | 0–11  | `centre`       | the bounding sphere's centre less the page's origin, m, `f32`          |
 * | 12–15 | `radius`       | its radius, m, `f32`, padded for the `f32` arithmetic that places it   |
 * | 16–23 | `errors`       | eight bytes read by the record's mode (below)                          |
 * | 24–27 | `templateRef`  | terrain: the block's or strip's place; a mesh: its triangles (below)   |
 * | 28–31 | `flags`        | shape, mode, may-discard and material (below)                          |
 *
 * - **The strided mode** (terrain, from plan R14's T4 the smooth figure): the cluster is a block of
 *   a regular grid drawn at a stride of 1, 2, 4 or 8, and its error at stride s is a unit, the
 *   instance's (terrain: its level's largest vertex spacing, `vertexSpacing(a, L).maxM`), times
 *   the template's `errorScale`. Its error bytes hold (0, +∞) as two `f32`, unread, until measured
 *   errors fill them: a record marked measured ({@link CLUSTER_FLAG_MEASURED}, plan R14, R14.T3.b)
 *   holds its e₁, e₂, e₄ and e₈, metres, as four `f16` rounded up, in that order, so that each
 *   `u32` is a pair `unpack2x16float` reads, the lower stride in its low half, with no `enable f16`.
 * - **The DAG-cut mode** (authored and procedural meshes, plan R14's T4 and T5): the error bytes
 *   hold (own, parent), metres, as two `f32`, and the cluster is drawn where own ≤ tolerance <
 *   parent, each projected at its group's sphere, so that a monotone DAG draws one consistent cut
 *   with no traversal (Karis 2021, SIGGRAPH Advances, "A Deep Dive into Nanite Virtualized
 *   Geometry"). The record's sphere is its group's LOD sphere, which bounds the cluster too; the
 *   parent group's is its {@link ClusterLod}, 16 B beside it. The hulls' occluder faces are its
 *   first consumer (R14.T4.b), a one-level DAG of (0, +∞).
 * - **Terrain's template reference**: bits 0–7 the grid column x₀ and bits 8–15 the row y₀ of the
 *   block's or the strip's first vertex in the 65-grid, bits 16–17 a strip's edge (0 y = 0,
 *   1 x = 64, 2 y = 64, 3 x = 0, as `gpu/resources.ts` numbers them); a block's x₀ and y₀ are
 *   multiples of 8, a strip's run along its edge.
 * - **A mesh's template reference** ({@link meshTemplateRef}): bits 0–23 the cluster's first
 *   triangle in its page's triangles and bits 24–31 its triangle count, at most 128, as
 *   `tools/meshbuild`'s pages write it (`page.rs`).
 * - **Flags**: bits 0–3 the shape ({@link CLUSTER_SHAPE_CODE}), bit 4 the mode (0 strided, 1 DAG
 *   cut), bit 5 whether its fragments may discard (terrain's interior clusters never do: plan R14,
 *   R14.T1.a), bit 6 whether a strided record's errors are measured, bits 8–15 the consumer's
 *   material. The survey discard stays the edge patches' own
 *   (plan R14's design note on the visibility buffer); no flag assumes it moves.
 */

import type { Vec3 } from "../../geometry/vec3";

/** Bytes of one cluster's record, as WGSL's `ClusterRecord` lays it out. */
export const CLUSTER_RECORD_BYTES = 32;

/** How a record's error bytes are read (see the module's remarks). */
export type ClusterLodMode = "strided" | "dag";

/** A cluster's shape: the template family it is drawn by. */
export type ClusterShape = "terrain-block" | "terrain-skirt" | "mesh";

/** Each shape's code in a record's flags, bits 0–3. */
export const CLUSTER_SHAPE_CODE: Readonly<Record<ClusterShape, number>> = {
  "terrain-block": 0,
  "terrain-skirt": 1,
  mesh: 2,
};

/** Bit 4 of the flags: the DAG-cut mode (see the module's remarks). */
export const CLUSTER_FLAG_DAG = 1 << 4;

/** Bit 5 of the flags: whether a cluster's fragments may discard. */
export const CLUSTER_FLAG_MAY_DISCARD = 1 << 5;

/** Bit 6 of the flags: whether a strided record's error bytes hold its measured e_s. */
export const CLUSTER_FLAG_MEASURED = 1 << 6;

/** The bit pattern of `f32` +∞, a strided record's unread parent error. */
const F32_INFINITY_BITS = 0x7f80_0000;

/** What every cluster's record holds whatever its mode (see the module's remarks). */
interface ClusterRecordBase {
  /** The bounding sphere's centre less the page's origin, in the page's axes, m. */
  readonly centreM: Vec3;
  /** Its radius, m. */
  readonly radiusM: number;
  /** Its template reference (see the module's remarks). */
  readonly templateRef: number;
  readonly shape: ClusterShape;
  readonly mayDiscard: boolean;
  /** The consumer's material, 0 to 255. */
  readonly material: number;
}

/** One cluster's static record, by how its error bytes are read (see the module's remarks). */
export type ClusterRecord =
  | (ClusterRecordBase & {
      readonly mode: "strided";
      /**
       * Its measured e₁, e₂, e₄ and e₈ as the bits of half floats, rounded up when they were made
       * (`clusters/errors.ts`' `packClusterErrors`, plan R14, R14.T3.b); `null` where unmeasured,
       * whose bytes hold (0, +∞).
       */
      readonly strideErrorBits: readonly [number, number, number, number] | null;
    })
  | (ClusterRecordBase & {
      readonly mode: "dag";
      /** Its (own, parent) errors, m. */
      readonly errorsM: readonly [number, number];
    });

/** A record's flags word from its shape, mode, discard, measured errors and material. */
export function clusterFlags(record: ClusterRecord): number {
  return (
    CLUSTER_SHAPE_CODE[record.shape] |
    (record.mode === "dag" ? CLUSTER_FLAG_DAG : 0) |
    (record.mayDiscard ? CLUSTER_FLAG_MAY_DISCARD : 0) |
    (record.mode === "strided" && record.strideErrorBits !== null ? CLUSTER_FLAG_MEASURED : 0) |
    ((record.material & 0xff) << 8)
  );
}

/**
 * Writes `record` into `out` at `offsetBytes`, little-endian, each number narrowed once to `f32`.
 *
 * @throws RangeError for a template reference that is not a `u32`, a material outside 0 to 255,
 *   a measured error's bits that are not a `u16`, or a record that would not fit `out`.
 */
export function packClusterRecord(out: DataView, offsetBytes: number, record: ClusterRecord): void {
  if (offsetBytes < 0 || offsetBytes + CLUSTER_RECORD_BYTES > out.byteLength) {
    throw new RangeError(
      `a cluster record at ${String(offsetBytes)} B lies outside ${String(out.byteLength)} B`,
    );
  }
  if (
    !Number.isInteger(record.templateRef) ||
    record.templateRef < 0 ||
    record.templateRef > 0xffff_ffff
  ) {
    throw new RangeError(`a template reference of ${String(record.templateRef)} is not a u32`);
  }
  if (!Number.isInteger(record.material) || record.material < 0 || record.material > 0xff) {
    throw new RangeError(`a cluster's material ${String(record.material)} is not 0 to 255`);
  }
  out.setFloat32(offsetBytes, record.centreM.x, true);
  out.setFloat32(offsetBytes + 4, record.centreM.y, true);
  out.setFloat32(offsetBytes + 8, record.centreM.z, true);
  out.setFloat32(offsetBytes + 12, record.radiusM, true);
  switch (record.mode) {
    case "strided": {
      const bits = record.strideErrorBits;
      if (bits === null) {
        out.setUint32(offsetBytes + 16, 0, true);
        out.setUint32(offsetBytes + 20, F32_INFINITY_BITS, true);
        break;
      }
      for (const [n, b] of bits.entries()) {
        if (!Number.isInteger(b) || b < 0 || b > 0xffff) {
          throw new RangeError(`a measured error's bits ${String(b)} are not a u16`);
        }
        out.setUint16(offsetBytes + 16 + 2 * n, b, true);
      }
      break;
    }
    case "dag":
      out.setFloat32(offsetBytes + 16, record.errorsM[0], true);
      out.setFloat32(offsetBytes + 20, record.errorsM[1], true);
      break;
  }
  out.setUint32(offsetBytes + 24, record.templateRef, true);
  out.setUint32(offsetBytes + 28, clusterFlags(record), true);
}

/** Bytes of one cluster's {@link ClusterLod}, as WGSL's `vec4f` lays it out. */
export const CLUSTER_LOD_BYTES = 16;

/**
 * A DAG-cut record's parent group's LOD sphere (see the module's remarks), about the page's origin
 * in its axes, as the record's own sphere is: the sphere its parent error is projected at.
 */
export interface ClusterLod {
  readonly parentCentreM: Vec3;
  readonly parentRadiusM: number;
}

/** Writes `lod` into `out` at `offsetBytes`, little-endian, each number narrowed once to `f32`. */
export function packClusterLod(out: DataView, offsetBytes: number, lod: ClusterLod): void {
  if (offsetBytes < 0 || offsetBytes + CLUSTER_LOD_BYTES > out.byteLength) {
    throw new RangeError(
      `a cluster's LOD sphere at ${String(offsetBytes)} B lies outside ${String(out.byteLength)} B`,
    );
  }
  out.setFloat32(offsetBytes, lod.parentCentreM.x, true);
  out.setFloat32(offsetBytes + 4, lod.parentCentreM.y, true);
  out.setFloat32(offsetBytes + 8, lod.parentCentreM.z, true);
  out.setFloat32(offsetBytes + 12, lod.parentRadiusM, true);
}

/** The most triangles a mesh cluster holds: 7 bits of the visibility ID (plan R14, Design note 2). */
export const MESH_CLUSTER_TRIANGLES = 128;

/**
 * A mesh cluster's template reference (see the module's remarks): its first triangle in its
 * page's triangles and its triangle count.
 *
 * @throws RangeError for a first triangle past 24 bits, or a count outside 1 to
 *   {@link MESH_CLUSTER_TRIANGLES}.
 */
export function meshTemplateRef(firstTriangle: number, triangles: number): number {
  if (!Number.isInteger(firstTriangle) || firstTriangle < 0 || firstTriangle > 0xff_ffff) {
    throw new RangeError(`a mesh cluster's first triangle ${String(firstTriangle)} is not 24 bits`);
  }
  if (!Number.isInteger(triangles) || triangles < 1 || triangles > MESH_CLUSTER_TRIANGLES) {
    throw new RangeError(
      `a mesh cluster of ${String(triangles)} triangles is not 1 to ${String(MESH_CLUSTER_TRIANGLES)}`,
    );
  }
  return (firstTriangle | (triangles << 24)) >>> 0;
}
