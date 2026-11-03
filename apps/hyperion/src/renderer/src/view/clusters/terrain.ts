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
 *
 * **Fin clusters** (plan R14, R14.T1.g): after the strips, cluster 96 + 2 e + h for the skirt heads
 * of half h of edge e, from the edge's first vertex, drawn by the head run's template
 * (`terrain-head`, `clusters/templates.ts`), and 104 + 2 n + h for half h of fin line n, the
 * columns {@link FIN_LINES} then its rows (`gpu/resources.ts`' `SURVEY_FIN_LINES`), drawn by the
 * fin run's (`terrain-fin`). A run's sphere is the quarter patch's (f, L + 1, 2i + qi, 2j + qj)
 * that holds it, its range widened to the datum and by h_max − h_min each way, where a head or a
 * fin's datum end stands along its vertex's normal.
 *
 * **The survey** (plan R14, R14.T1.g): each record is classified by the slot's survey mask as the
 * lit fragment's `survey_kept_at` reads it (`terrainSurvey.wgsl`; `gpu/resources.ts`'
 * `surveyedAt`), over the cluster's footprint widened by {@link SURVEY_MARGIN_QUADS}: a block or a
 * strip every fragment of which is kept is opaque, as an interior patch's are; one none of which is
 * kept is no cluster (a negative radius); the rest may discard, and are drawn by the survey
 * discard's form. A head run is no cluster where no fragment of it is kept; a fin run where its
 * line is no cell edge of the survey's, where its datum ends collapse, or where the cells either
 * side of it agree along it; else it may discard. The survey alone decides, never the heights,
 * which a rebake's or a rise's fade mixes from another surface (plan R10, R10.T16.i). An interior
 * patch's records are its blocks and strips, opaque, and no fin cluster.
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
  FIN_RUN_QUADS,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_FIN_CLUSTERS,
  TERRAIN_HEAD_CLUSTERS,
  TERRAIN_PAGE_CLUSTERS,
} from "./terrainGrid";

export {
  CLUSTER_PAD_FRACTION,
  CLUSTER_PAD_M,
  edgeVertex,
  TERRAIN_BLOCKS,
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_FIN_CLUSTERS,
  TERRAIN_HEAD_CLUSTERS,
  TERRAIN_PAGE_CLUSTERS,
} from "./terrainGrid";

/** Quads along a patch's side. */
const PATCH_QUADS = 64;

/** Blocks along a patch's side, and strips along an edge. */
const BLOCKS_PER_SIDE = PATCH_QUADS / CLUSTER_QUADS;

/** A strip's further padding per metre of its height span (see the module's remarks). */
export const STRIP_NORMAL_FRACTION = 1e-3;

/**
 * The columns and rows of a patch that carry a survey fin (R10.T10.c): `gpu/resources.ts`'
 * `SURVEY_FIN_LINES`, made here, which that module imports.
 */
export const FIN_LINES: readonly [number, number, number] = [16, 32, 48];

/**
 * How far past a cluster's footprint its survey is read, quads (see the module's remarks): half a
 * quad, past any rounding of a fragment's interpolated place (a few `f32` ulps of 64).
 */
export const SURVEY_MARGIN_QUADS = 0.5;

/** The lit mesh's vertex codes a fin run's vertices take (`gpu/resources.ts`' `LIT_VERTEX_CODE`). */
const LIT_GRID = 0;
const LIT_SKIRT_HEAD = 2;
const LIT_COLUMN_FIN = 4;
const LIT_ROW_FIN = 5;

/** Bytes of a slot's survey mask before its cell code (`gpu/resources.ts`' `SURVEY_CELL_CODE_BYTE`). */
const SURVEY_CELL_CODE_BYTE = 65 * 65;

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

/** A run of an edge's skirt heads (see the module's remarks). */
export interface TerrainHeadRun {
  readonly line: "head";
  /** Its edge, 0 to 3, as `gpu/resources.ts` numbers them. */
  readonly edge: number;
  /** Its half of the edge, 0 from the edge's first vertex. */
  readonly half: 0 | 1;
}

/** A run of a survey fin along a column (x constant) or a row (y constant). */
export interface TerrainFinRun {
  readonly line: "column" | "row";
  /** The fin's column or row, one of {@link FIN_LINES}. */
  readonly at: number;
  /** Its half of the line, 0 from the line's first vertex. */
  readonly half: 0 | 1;
}

/** Where fin cluster `cluster` lies: a head run or a fin run (see the module's remarks). */
export type TerrainFinPlace = TerrainHeadRun | TerrainFinRun;

/**
 * Fin cluster `cluster`'s place, {@link TERRAIN_CLUSTERS_PER_PAGE} to
 * {@link TERRAIN_PAGE_CLUSTERS} − 1.
 *
 * @throws RangeError for any other cluster.
 */
export function terrainFinPlace(cluster: number): TerrainFinPlace {
  const n = cluster - TERRAIN_CLUSTERS_PER_PAGE;
  if (!Number.isInteger(cluster) || n < 0 || n >= TERRAIN_FIN_CLUSTERS) {
    throw new RangeError(`a terrain patch has no fin cluster ${String(cluster)}`);
  }
  const half = n % 2 === 0 ? 0 : 1;
  if (n < TERRAIN_HEAD_CLUSTERS) {
    return { line: "head", edge: Math.floor(n / 2), half };
  }
  const line = Math.floor((n - TERRAIN_HEAD_CLUSTERS) / 2);
  return {
    line: line < FIN_LINES.length ? "column" : "row",
    at: FIN_LINES[line % FIN_LINES.length] ?? 0,
    half,
  };
}

/** The grid vertex k quads along fin place `place`'s run, from its first vertex. */
function finRunVertex(place: TerrainFinPlace, k: number): readonly [number, number] {
  const along = FIN_RUN_QUADS * place.half + k;
  let vertex: readonly [number, number];
  switch (place.line) {
    case "head":
      vertex = edgeVertex(place.edge, along);
      break;
    case "column":
      vertex = [place.at, along];
      break;
    case "row":
      vertex = [along, place.at];
      break;
  }
  return vertex;
}

/**
 * The record's template reference of fin cluster `cluster` (`clusters/record.ts`' remarks): its
 * first vertex's column and row, a head run's edge, a row's bit and the fin run's bit.
 */
export function terrainFinTemplateRef(cluster: number): number {
  const place = terrainFinPlace(cluster);
  const [x0, y0] = finRunVertex(place, 0);
  const tail =
    place.line === "head" ? place.edge << 16 : (1 << 19) | (place.line === "row" ? 1 << 18 : 0);
  return (x0 | (y0 << 8) | tail) >>> 0;
}

/**
 * The lit mesh's grid coordinates and code of a fin or head run's template vertex `local`
 * ((k, 0, kind), the terrain cluster mesh's `position`) of fin cluster `cluster`: the vertex
 * stage's decode (`terrainCluster.wgsl`'s `terrain_fin_grid`). A grid vertex (kind 0) is the run's
 * k-th; its datum vertex (kind 1) the lit mesh's head or fin vertex there.
 */
export function terrainFinGrid(
  cluster: number,
  local: readonly [number, number, number],
): readonly [number, number, number] {
  const place = terrainFinPlace(cluster);
  const [k, , kind] = local;
  const [x, y] = finRunVertex(place, k);
  if (kind === 0) {
    return [x, y, LIT_GRID];
  }
  const code =
    place.line === "head" ? LIT_SKIRT_HEAD : place.line === "column" ? LIT_COLUMN_FIN : LIT_ROW_FIN;
  return [x, y, code];
}

/**
 * The sub-patch word's bit (terrain's `Instance.sub.x`, bit 31; `gpu/uniforms.ts`' `subPatchWord`
 * holds bits 0 to 30) of a stand-in drawn as a mask of its slot's patch's own blocks (plan R14,
 * Design note 19): a missing patch `levels` (1 to {@link TERRAIN_MASK_LEVELS}) below the patch A
 * whose slot draws it, at part (i, j), draws A's blocks over its area, a 4 × 4, 2 × 2 or 1 × 1 set,
 * on A's own grid, and its strips, 8 a side as an edge's are, along the set's sides: A's own where a
 * side lies on A's edge, else a foot strip on A's grid line there, where the lit mesh has no skirt.
 */
export const TERRAIN_SUB_PATCH_MASK = 0x8000_0000;

/** The most levels below its slot's patch a stand-in is drawn as a mask at. */
export const TERRAIN_MASK_LEVELS = 3;

/**
 * How a terrain instance of sub-patch word `subWord` draws its page's cluster `cluster`
 * (`clusterCull.wgsl`'s `mask_cluster`): 1, as its sphere says, for every cluster of an instance
 * that is not a mask ({@link TERRAIN_SUB_PATCH_MASK}) and for a mask's own blocks; 2, whatever its
 * sphere says, for one of a mask's strips, the first (8 ÷ 2^levels) of each side's, which its
 * sphere, an edge strip's, does not bound; 0 for any other.
 */
export function terrainMaskCluster(subWord: number, cluster: number): 0 | 1 | 2 {
  if ((subWord & TERRAIN_SUB_PATCH_MASK) === 0) {
    return 1;
  }
  const side = BLOCKS_PER_SIDE >> (subWord & 31);
  const bx0 = ((subWord >>> 5) & 0x1fff) * side;
  const by0 = ((subWord >>> 18) & 0x1fff) * side;
  if (cluster < TERRAIN_BLOCKS) {
    const bx = cluster % BLOCKS_PER_SIDE;
    const by = Math.floor(cluster / BLOCKS_PER_SIDE);
    return bx >= bx0 && bx < bx0 + side && by >= by0 && by < by0 + side ? 1 : 0;
  }
  if (cluster < TERRAIN_CLUSTERS_PER_PAGE) {
    return (cluster - TERRAIN_BLOCKS) % BLOCKS_PER_SIDE < side ? 2 : 0;
  }
  return 0;
}

/**
 * The lit mesh's grid coordinates and code of a template vertex `local` ((lx, ly, code), the
 * terrain cluster mesh's `position`) of cluster `cluster` of an instance of sub-patch word
 * `subWord`: the vertex stage's decode, which hands `lit_place` exactly the lit mesh's own vertex
 * (`terrainCluster.wgsl`'s `terrain_cluster_grid`), a mask's strips along its set's sides
 * ({@link TERRAIN_SUB_PATCH_MASK}).
 */
export function terrainClusterGrid(
  cluster: number,
  local: readonly [number, number, number],
  subWord = 0,
): readonly [number, number, number] {
  const place = terrainClusterPlace(cluster);
  const [lx, ly, code] = local;
  if (place.shape === "terrain-block") {
    return [CLUSTER_QUADS * place.bx + lx, CLUSTER_QUADS * place.by + ly, code];
  }
  const k = CLUSTER_QUADS * place.strip + lx;
  if ((subWord & TERRAIN_SUB_PATCH_MASK) !== 0) {
    const quads = PATCH_QUADS >> (subWord & 31);
    const x0 = ((subWord >>> 5) & 0x1fff) * quads;
    const y0 = ((subWord >>> 18) & 0x1fff) * quads;
    // The set's sides anticlockwise, as `edgeVertex` takes a patch's.
    const at: ReadonlyArray<readonly [number, number]> = [
      [x0 + k, y0],
      [x0 + quads, y0 + k],
      [x0 + quads - k, y0 + quads],
      [x0, y0 + quads - k],
    ];
    const [x, y] = at[place.edge] ?? [x0, y0];
    return [x, y, code];
  }
  const [x, y] = edgeVertex(place.edge, k);
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
  /**
   * The slot's survey mask as its `survey-mask` field holds it (`gpu/resources.ts`'
   * `packSurveyMask`), by which the records are classified (plan R14, R14.T1.g; see the module's
   * remarks); absent, every block and strip is opaque and no fin cluster is drawn, an interior
   * patch's records.
   */
  readonly surveyMask?: Uint8Array;
}

/**
 * How the survey keeps a cluster's fragments (see the module's remarks): every one, some, or none,
 * where it is no cluster.
 */
export type ClusterSurvey = "whole" | "edge" | "none";

/**
 * Whether the survey mask `packed` ({@link TerrainClusterSource.surveyMask}) may hold the ground
 * surveyed and unsurveyed anywhere in [x₀, x₁] × [y₀, y₁], quads from the patch's corner, as
 * `survey_surveyed` reads it: each coarse cell meeting the rectangle at its centre vertex, or, where
 * a cell spans a quad or less, the bilinear mask over each quad meeting it, which lies between its
 * four corners'.
 */
function surveyStates(
  packed: Uint8Array,
  x0: number,
  x1: number,
  y0: number,
  y1: number,
): { readonly surveyed: boolean; readonly unsurveyed: boolean } {
  const code = Math.min(packed[SURVEY_CELL_CODE_BYTE] ?? 0, 6);
  const at = (x: number, y: number): number => (packed[y * 65 + x] ?? 0) / 255;
  let surveyed = false;
  let unsurveyed = false;
  if (code === 0) {
    const base = (f: number): number => Math.min(PATCH_QUADS - 1, Math.floor(Math.max(f, 0)));
    for (let by = base(y0); by <= base(y1); by += 1) {
      for (let bx = base(x0); bx <= base(x1); bx += 1) {
        const corners = [at(bx, by), at(bx + 1, by), at(bx, by + 1), at(bx + 1, by + 1)];
        surveyed ||= Math.max(...corners) >= 0.5;
        unsurveyed ||= Math.min(...corners) < 0.5;
      }
    }
    return { surveyed, unsurveyed };
  }
  const quads = 2 ** code;
  const cells = Math.max(1, PATCH_QUADS / quads);
  const cell = (f: number): number => Math.min(cells - 1, Math.floor(Math.max(f, 0) / quads));
  for (let cy = cell(y0); cy <= cell(y1); cy += 1) {
    for (let cx = cell(x0); cx <= cell(x1); cx += 1) {
      const centre = (c: number): number => Math.min(PATCH_QUADS - 1, c * quads + quads / 2);
      if (at(centre(cx), centre(cy)) >= 0.5) {
        surveyed = true;
      } else {
        unsurveyed = true;
      }
    }
  }
  return { surveyed, unsurveyed };
}

/** A footprint's rectangle in quads, [x₀, x₁] × [y₀, y₁]. */
type Footprint = readonly [x0: number, x1: number, y0: number, y1: number];

/** The rectangle spanning grid vertices `a` and `b`, widened by {@link SURVEY_MARGIN_QUADS}. */
function footprintOf(a: readonly [number, number], b: readonly [number, number]): Footprint {
  const m = SURVEY_MARGIN_QUADS;
  return [
    Math.min(a[0], b[0]) - m,
    Math.max(a[0], b[0]) + m,
    Math.min(a[1], b[1]) - m,
    Math.max(a[1], b[1]) + m,
  ];
}

/** A skirt's footprint as its survey reads it: clamped half a quad inside the patch. */
function skirtFootprint(footprint: Footprint): Footprint {
  const c = (f: number): number => Math.min(PATCH_QUADS - 0.5, Math.max(0.5, f));
  return [c(footprint[0]), c(footprint[1]), c(footprint[2]), c(footprint[3])];
}

/** How a cluster kept where the ground is surveyed (or, `inverse`, where not) is kept over `footprint`. */
function keptOver(packed: Uint8Array, footprint: Footprint, inverse: boolean): ClusterSurvey {
  const { surveyed, unsurveyed } = surveyStates(packed, ...footprint);
  const kept = inverse ? unsurveyed : surveyed;
  const cut = inverse ? surveyed : unsurveyed;
  return !kept ? "none" : cut ? "edge" : "whole";
}

/**
 * How the survey mask `packed` keeps cluster `cluster` of a patch (see the module's remarks), every
 * cluster of {@link TERRAIN_PAGE_CLUSTERS}: by the survey alone, so that a fade of its heights
 * (R10.T16.i) leaves it as it is.
 */
export function terrainClusterSurvey(packed: Uint8Array, cluster: number): ClusterSurvey {
  if (cluster < TERRAIN_BLOCKS) {
    const { bx, by } = terrainClusterPlace(cluster);
    const x = CLUSTER_QUADS * bx;
    const y = CLUSTER_QUADS * by;
    return keptOver(packed, footprintOf([x, y], [x + CLUSTER_QUADS, y + CLUSTER_QUADS]), false);
  }
  if (cluster < TERRAIN_CLUSTERS_PER_PAGE) {
    const { edge, strip } = terrainClusterPlace(cluster);
    const from = edgeVertex(edge, CLUSTER_QUADS * strip);
    const to = edgeVertex(edge, CLUSTER_QUADS * (strip + 1));
    return keptOver(packed, skirtFootprint(footprintOf(from, to)), false);
  }
  const place = terrainFinPlace(cluster);
  const from = finRunVertex(place, 0);
  const to = finRunVertex(place, FIN_RUN_QUADS);
  if (place.line === "head") {
    // A head stands at max(h, 0), which a rebake's or a rise's fade mixes from another surface
    // (R10.T16.i): only the survey decides it.
    return keptOver(packed, skirtFootprint(footprintOf(from, to)), true) === "none"
      ? "none"
      : "edge";
  }
  const code = Math.min(packed[SURVEY_CELL_CODE_BYTE] ?? 0, 6);
  if (code === 0 || code >= 6 || place.at % 2 ** code !== 0) {
    return "none";
  }
  // Along the line the cells either side of it, row by row of cells (a column's) or column by
  // column (a row's): a fin draws where they differ.
  const [x0, x1, y0, y1] = footprintOf(from, to);
  const quads = 2 ** code;
  const cells = PATCH_QUADS / quads;
  const cell = (f: number): number => Math.min(cells - 1, Math.floor(Math.max(f, 0) / quads));
  const [lo, hi] = place.line === "column" ? [y0, y1] : [x0, x1];
  for (let c = cell(lo); c <= cell(hi); c += 1) {
    const middle = c * quads + quads / 2;
    const side = (offset: number): boolean =>
      place.line === "column"
        ? surveyStates(packed, place.at + offset, place.at + offset, middle, middle).surveyed
        : surveyStates(packed, middle, middle, place.at + offset, place.at + offset).surveyed;
    if (side(-0.5) !== side(0.5)) {
      return "edge";
    }
  }
  return "none";
}

/**
 * Writes a terrain patch's {@link TERRAIN_PAGE_CLUSTERS} records into `out` from `offsetBytes`,
 * cluster by cluster (see the module's remarks): each sphere about the bake's origin, in the
 * body-fixed axes, strided, with the bake's measured errors where it has them, opaque, may-discard
 * or no cluster by the slot's survey (plan R14, R14.T1.g).
 *
 * @throws RangeError for measured errors that are not four a cluster of
 *   {@link TERRAIN_CLUSTERS_PER_PAGE}.
 */
export function writeTerrainClusterRecords(
  planet: PlanetGeometry,
  source: TerrainClusterSource,
  out: DataView,
  offsetBytes: number,
): void {
  const { key, originM, heightRangeM, skirtDepthM, clusterErrors, surveyMask } = source;
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
    const survey = surveyMask === undefined ? "whole" : terrainClusterSurvey(surveyMask, cluster);
    packClusterRecord(out, offsetBytes + cluster * CLUSTER_RECORD_BYTES, {
      centreM: sphere.centreM,
      radiusM: survey === "none" ? -1 : sphere.radiusM,
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
      mayDiscard: survey === "edge",
      material: 0,
    });
  }
  // The fin clusters' spheres: their quarter patch's, from the datum's floor to its ceiling.
  const lowest = Math.min(low, 0) - (high - low);
  const highest = Math.max(high, 0) + (high - low);
  for (let cluster = TERRAIN_CLUSTERS_PER_PAGE; cluster < TERRAIN_PAGE_CLUSTERS; cluster += 1) {
    const survey = surveyMask === undefined ? "none" : terrainClusterSurvey(surveyMask, cluster);
    const [qi, qj] = finQuarter(terrainFinPlace(cluster));
    const quarter = patchBounds(
      planet,
      { face: key.face, level: key.level + 1, i: 2 * key.i + qi, j: 2 * key.j + qj },
      [lowest, highest],
    );
    const centreM = {
      x: quarter.centre.x - originM.x,
      y: quarter.centre.y - originM.y,
      z: quarter.centre.z - originM.z,
    };
    const reachM = Math.hypot(centreM.x, centreM.y, centreM.z) + quarter.radiusM;
    const radiusM =
      quarter.radiusM +
      CLUSTER_PAD_M +
      CLUSTER_PAD_FRACTION * reachM +
      STRIP_NORMAL_FRACTION * (highest - lowest);
    packClusterRecord(out, offsetBytes + cluster * CLUSTER_RECORD_BYTES, {
      centreM,
      radiusM: survey === "none" ? -1 : radiusM,
      mode: "strided",
      strideErrorBits: null,
      templateRef: terrainFinTemplateRef(cluster),
      shape: terrainFinPlace(cluster).line === "head" ? "terrain-head" : "terrain-fin",
      mayDiscard: true,
      material: 0,
    });
  }
}

/** The quarter of its patch, (qi, qj), that holds fin place `place`'s run, its line on its side. */
function finQuarter(place: TerrainFinPlace): readonly [number, number] {
  const [x0, y0] = finRunVertex(place, 0);
  const [x1, y1] = finRunVertex(place, FIN_RUN_QUADS);
  // The run's midpoint lies inside its quarter, or on the line between two, either of which holds it.
  const quarterOf = (f: number): number => (f <= PATCH_QUADS / 2 ? 0 : 1);
  return [quarterOf((x0 + x1) / 2), quarterOf((y0 + y1) / 2)];
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
