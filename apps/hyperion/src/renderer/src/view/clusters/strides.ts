/**
 * The strides of terrain's camera passes (plan R14, R14.T3.c; Design note 3): the bands each view's
 * clusters take strides 2, 4 and 8 at, the cull's choice of a cluster's stride by them, and the
 * stride morph that places every vertex so that a stride flips with nothing moving, each the
 * TypeScript twin of its WGSL (`clusterCull.wgsl`'s `camera_stride_level`, `terrainStrides.wgsl`'s
 * `stride_place`).
 *
 * @remarks
 * - **The stated-bound rule.** A camera pass draws a level-L patch's cluster at stride s only where
 *   the level's stated error E_L (`selectionErrorM`) plus e_s, projected at the cluster's own
 *   distance by `screenSpaceErrorPx`'s one function, is within the view's τ′ (Design note 3's,
 *   `effectiveTauPx(τ, limitExcess, w)`, reached a step at a time as the morph bands' is, so that a
 *   jump of the budget's τ′, up to 1.8 times a frame on the seamless probe, re-morphs no stride at
 *   once): at distances of at least A(L, s) = (E_L + e_s) ÷
 *   τ′_rad, τ′_rad = τ′ ÷ (W ÷ (2 tan(fov_h ÷ 2))), a cluster's need ({@link strideNeedM}, of its
 *   patch's {@link StrideScale}).
 * - **The shared bands** (fix-r14-t3c-shared-bands, research-leans-35's ruling, 2026-11-03): on a
 *   patch's edge a vertex's band towards 2^(j + 1) is the edge's shared band there, the largest need
 *   of its own block and the neighbour's adjacent blocks (a fine patch's level j + 1 against a
 *   coarse one's level j, over each coarse block's stretch, the coarse–fine holds kept), or of its
 *   block and the level's largest e_s where the neighbour is not a measured whole patch
 *   (`terrain/strideEdges.ts`). A block's band is the largest need of it, the blocks on its sides
 *   and its edges' shared bands along it ({@link clusterStrideBandsM}), none where any of them is,
 *   and a cluster takes stride 2^m at (1 + β) times it. Inside a block a vertex's band is its
 *   block's of its parts that are not none, and on a block's side the smaller of its two blocks'
 *   ({@link vertexStrideBandM}): at most the band of each block that drops it, so that each draws it
 *   wholly moved, and at least the need of each block holding it. So every vertex two clusters draw
 *   is placed alike by both, whichever patch each is of. A vertex on a block's side or a patch's
 *   edge may still move before the block of the larger band drops it, so a block's mesh can mix
 *   two strides there: the drawn error within τ′ is what `strides.wasm.test.ts` finds, not a bound
 *   of the rule.
 * - **Why a block's band inside it** (as built, 2026-11-03; the ruling gave each vertex the largest
 *   need of the blocks holding it): e_s bounds a block's stride-s mesh with one k across it (Design
 *   note 3). A vertex the block keeps but that moves at its own block's need, while a neighbour's
 *   larger need holds the block at the finer stride, leaves a mesh that is neither stride's: on the
 *   Moon-like world's bakes the drawn-error test (`strides.wasm.test.ts`) drew 1.08 τ′ at a block
 *   held at stride 4 whose level-2 vertices had moved to stride 8's chords. At its block's band a
 *   vertex moves only as its block is about to drop it, as
 *   under the view's ladder, at no cost to the cull's strides. A block's side still moves at the
 *   smaller band of its two blocks, and a patch's edge at the larger need of its blocks. An edge's
 *   none (beside a finer patch at level 2, or two levels apart) holds its block at a finer stride
 *   but not the vertices inside it: held, they would all move at once as the finer patch left, so
 *   there they move at their block's other parts' band and the block's mesh mixes its strides, as
 *   under the ruling.
 * - **The records.** The blocks' errors reach the vertex stage in their slot's record and the
 *   edges' bands in the instance's, as half floats of kilometres rounded up
 *   ({@link edgeBandHalf}).
 * - **The stride morph** (CDLOD's distance morph, Strugar 2010, applied to the strides; stateless):
 *   a vertex at morph level j, its grid place a multiple of 2^j but not of 2^(j + 1) (capped at 3,
 *   a block's corners, which never move), is what the stride-2^(j + 1) mesh drops; it moves towards
 *   the mean of its two neighbours at 2^j along its quad's side or on its (0, 0)–(1, 1) diagonal
 *   ({@link strideParents}), the coarser mesh's interpolant there, by k_j = clamp((d − U) ÷ (β U),
 *   0, 1), d its own distance and U its band ({@link STRIDE_BAND_FRACTION};
 *   {@link vertexStrideBandM}). The neighbours are placed so themselves, so a vertex's place is a
 *   function of the vertex, its patch's errors and scale and its edges' bands alone
 *   ({@link stridePlace}), and a cluster drawn at stride 2^m, every vertex it drops at k = 1,
 *   draws exactly the morphed surface: the cull takes 2^m only where the cluster's nearest point is
 *   past (1 + β) U of each of its bands up to stride 2^m ({@link cameraStrideLevel}), so a stride
 *   flipping there moves nothing, at any stride beside any other. A vertex's neighbours may be
 *   moving as it moves, which the evaluation of each in turn holds exact. The morph mixes the
 *   vertex's height, its CDLOD k and its datum's normal with its place, so that the varyings flip
 *   with it.
 * - **Where it holds.** A contact's hold (R05 Design note 6) caps each k at the vertex's hold, so
 *   the ground within every held radius is the finest interpolant (Design note 10); where a finer
 *   patch touches an edge (its coarse–fine hold at k = 0), the edge's level-2 vertices stay, since
 *   the finer patch's vertices there are its block corners, falling to it over
 *   {@link STRIDE_HOLD_FADE_MS} as the hold switches on; and where a coarser one does (its hold
 *   at k = 1), the edge's level-0 vertices, at the coarser grid's midpoints, move wholly, so that
 *   they stand on its edge wherever its vertices move ({@link strideHeldFactor}).
 */

import type { PlanetGeometry } from "../terrain/planet";
import { selectionErrorM } from "../terrain/select";

/**
 * β, a stride's band's width over its start: a vertex dropped at stride 2s morphs over the
 * distances U to (1 + β) U, and the cull takes 2s at (1 + β) U (a hand value, lean, 2026-11-03).
 * Under the shared bands many seen blocks stand within a few per cent of a band, so β sets most of
 * the lit triangles drawn past the bands' own share: on the seamless probe's 0–70 s on high 0.122
 * of stride 1's at 0.02, 0.0456 at 0.005 and 0.0325 at 0.002, against 0.027 with no band, its moves
 * unchanged (the lane's outbox). The widest within the restated 0.031 + 10%.
 */
export const STRIDE_BAND_FRACTION = 0.002;

/** A band no stride is taken at, and no vertex morphs by: any value at or below 0. */
export const NO_STRIDE_BAND = -1;

/** The morph levels a vertex may move at, 0 to 2, towards strides 2, 4 and 8; 3 never moves. */
export const STRIDE_MORPH_LEVELS = 3;

/**
 * Bit 18 of terrain's instance record's quadrant word (`gpu/uniforms.ts`' `InstanceRecords`): every
 * camera pass draws the patch's clusters at stride 1, the forced region's (Design note 10). Its
 * vertices still take the stride morph, held at each contact as the CDLOD morph is.
 */
export const INSTANCE_FINEST = 1 << 18;

/** A patch's or an edge's three bands, m: towards strides 2, 4 and 8. */
export type StrideBandsM = readonly [two: number, four: number, eight: number];

/** No strides: every band {@link NO_STRIDE_BAND}. */
export const NO_STRIDE_BANDS: StrideBandsM = [NO_STRIDE_BAND, NO_STRIDE_BAND, NO_STRIDE_BAND];

/** A cluster's or a level's e₂, e₄ and e₈, m. */
export type StrideErrorsM = readonly [two: number, four: number, eight: number];

/**
 * The largest e₂, e₄ and e₈ of a bake's clusters, m (`BakedPatch.clusterErrors`, four half floats a
 * cluster, e₁ first), +∞ for a NaN.
 */
export function largestStrideErrorsM(clusterErrors: Float16Array): StrideErrorsM {
  const most = [0, 0, 0];
  for (let c = 0; 4 * c + 3 < clusterErrors.length; c += 1) {
    for (let l = 0; l < 3; l += 1) {
      const e = clusterErrors[4 * c + l + 1] ?? Number.POSITIVE_INFINITY;
      most[l] = Math.max(most[l] ?? 0, Number.isNaN(e) ? Infinity : e);
    }
  }
  return [most[0] ?? Infinity, most[1] ?? Infinity, most[2] ?? Infinity];
}

/**
 * A patch's stride scale at its view's tolerance (see the module's remarks): a cluster whose error
 * towards a stride is e needs the band statedM + e × perErrorM, m, A(L, s) = (E_L + e_s) ÷ τ′_rad.
 */
export interface StrideScale {
  /** E_L ÷ τ′_rad, m. */
  readonly statedM: number;
  /** 1 ÷ τ′_rad, m of band a metre of error; at or below 0 for none, its patch never striding. */
  readonly perErrorM: number;
}

/** No strides: no band. */
export const NO_STRIDE_SCALE: StrideScale = { statedM: 0, perErrorM: 0 };

/**
 * A level-`level` patch's scale at the view's tolerance `toleranceRad` (τ′ over its px per radian):
 * {@link NO_STRIDE_SCALE} where the tolerance is not above 0 or not finite.
 */
export function patchStrideScale(
  planet: PlanetGeometry,
  level: number,
  toleranceRad: number,
): StrideScale {
  if (!(toleranceRad > 0 && Number.isFinite(toleranceRad))) {
    return NO_STRIDE_SCALE;
  }
  return { statedM: selectionErrorM(planet, level) / toleranceRad, perErrorM: 1 / toleranceRad };
}

/**
 * The band, m, a cluster of error `errorM` towards a stride needs under `scale`, in `f32` as the
 * kernel and the vertex stage form it (`clusterCull.wgsl`'s `stride_need`, `terrainStrides.wgsl`'s
 * own); {@link NO_STRIDE_BAND} for no scale or an error not finite and at least 0.
 */
export function strideNeedM(scale: StrideScale, errorM: number): number {
  const f = Math.fround;
  if (!(scale.perErrorM > 0) || !(errorM >= 0) || !Number.isFinite(errorM)) {
    return NO_STRIDE_BAND;
  }
  return f(f(scale.statedM) + f(f(errorM) * f(scale.perErrorM)));
}

/** Band `a` and band `b` shared: the larger, none where either is none. */
export function sharedStrideBandM(a: number, b: number): number {
  return a > 0 && b > 0 ? Math.max(a, b) : NO_STRIDE_BAND;
}

/** Bands `a` and `b` shared, stride by stride ({@link sharedStrideBandM}). */
export function sharedStrideBandsM(a: StrideBandsM, b: StrideBandsM): StrideBandsM {
  return [
    sharedStrideBandM(a[0], b[0]),
    sharedStrideBandM(a[1], b[1]),
    sharedStrideBandM(a[2], b[2]),
  ];
}

/**
 * A block's error towards stride 2^(level + 1), m, of a bake's clusters' errors (`BakedPatch.
 * clusterErrors`, four half floats a cluster, e₁ first), +∞ for a NaN or one missing.
 */
export function blockErrorM(
  clusterErrors: ArrayLike<number>,
  block: number,
  level: number,
): number {
  const e = clusterErrors[4 * block + level + 1] ?? Number.POSITIVE_INFINITY;
  return Number.isNaN(e) ? Number.POSITIVE_INFINITY : e;
}

/** The bands a patch's edges share (see the module's remarks): 4 edges × 8 blocks × 3 strides. */
export const EDGE_BANDS = 96;

/**
 * The place in a patch's edges' bands of edge `edge` (`EDGES`' order: x = 0, x = 64, y = 0, y = 64)
 * at its block `t` along it (y ÷ 8 on x = 0 and 64, x ÷ 8 on y = 0 and 64), towards stride
 * 2^(level + 1).
 */
export function edgeBandIndex(edge: number, t: number, level: number): number {
  return (8 * edge + t) * 3 + level;
}

/** A patch's edges' shared bands, m, {@link EDGE_BANDS} of them at {@link edgeBandIndex}. */
export type StrideEdgeBandsM = ArrayLike<number>;

/** No edge's bands: every edge's vertices held. */
export const NO_STRIDE_EDGE_BANDS: StrideEdgeBandsM = new Float64Array(EDGE_BANDS).fill(
  NO_STRIDE_BAND,
);

/** The `Float16Array` scratch the edges' bands are rounded through. */
const HALF = new Float16Array(1);
const HALF_BITS = new Uint16Array(HALF.buffer);

/**
 * An edge's shared band as the instance record holds it (`gpu/uniforms.ts`): a half float of
 * kilometres, rounded up, so that no stride is taken nearer than its need; a negative half for none
 * and +∞ for a band past 65,504 km.
 */
export function edgeBandHalf(bandM: number): number {
  if (!(bandM > 0)) {
    return 0xbc00; // −1
  }
  const km = bandM / 1000;
  HALF[0] = km;
  if (HALF[0] < km && (HALF_BITS[0] ?? 0) < 0x7c00) {
    HALF_BITS[0] = (HALF_BITS[0] ?? 0) + 1;
  }
  return HALF_BITS[0] ?? 0x7c00;
}

/**
 * The band, m, of an edge's half float of kilometres, `halfKmBits` its bits ({@link edgeBandHalf}),
 * in `f32` as the kernel and the vertex stage read it (`unpack2x16float` × 1000),
 * {@link NO_STRIDE_BAND} for a negative one.
 */
export function edgeBandMOfHalf(halfKmBits: number): number {
  HALF_BITS[0] = halfKmBits;
  const km = HALF[0] ?? -1;
  return km > 0 ? Math.fround(Math.fround(km) * 1000) : NO_STRIDE_BAND;
}

/** `bandsM` as the instance record holds them and the shaders read them back, m. */
export function heldEdgeBandsM(bandsM: StrideEdgeBandsM): Float64Array {
  return Float64Array.from({ length: EDGE_BANDS }, (_, n) =>
    edgeBandMOfHalf(edgeBandHalf(bandsM[n] ?? NO_STRIDE_BAND)),
  );
}

/**
 * Block (bx, by)'s band towards stride 2^(level + 1) as the vertices inside it take it, m (see the
 * module's remarks): its band ({@link clusterStrideBandsM}) of its parts that are not none, so that
 * an edge that holds the block at a finer stride holds no vertex inside it; none where its own need
 * is (`terrainStrides.wgsl`'s `stride_block_band`).
 */
function blockVertexBandM(
  scale: StrideScale,
  clusterErrors: ArrayLike<number>,
  edgeBandsM: StrideEdgeBandsM,
  bx: number,
  by: number,
  level: number,
): number {
  const need = (block: number): number =>
    strideNeedM(scale, blockErrorM(clusterErrors, block, level));
  const edgeBand = (edge: number, t: number): number =>
    edgeBandsM[edgeBandIndex(edge, t, level)] ?? NO_STRIDE_BAND;
  const block = 8 * by + bx;
  const own = need(block);
  if (!(own > 0)) {
    return NO_STRIDE_BAND;
  }
  const parts = [
    bx > 0 ? need(block - 1) : edgeBand(0, by),
    bx < 7 ? need(block + 1) : edgeBand(1, by),
    by > 0 ? need(block - 8) : edgeBand(2, bx),
    by < 7 ? need(block + 8) : edgeBand(3, bx),
  ];
  return parts.reduce((band, part) => (part > 0 ? Math.max(band, part) : band), own);
}

/**
 * The band, m, of a patch's grid vertex (x, y) at morph level `level` (below 3; see the module's
 * remarks): on a patch's edge (not a corner, which is level 3) its edge's shared band there; on a
 * block's side the smaller of its two blocks' bands as their vertices take them, none where either
 * block's need is; else its block's (`terrainStrides.wgsl`'s `stride_band`).
 */
export function vertexStrideBandM(
  scale: StrideScale,
  clusterErrors: ArrayLike<number>,
  edgeBandsM: StrideEdgeBandsM,
  x: number,
  y: number,
  level: number,
): number {
  if (x === 0 || x === 64 || y === 0 || y === 64) {
    const edge = x === 0 ? 0 : x === 64 ? 1 : y === 0 ? 2 : 3;
    const t = Math.floor((edge < 2 ? y : x) / 8);
    return edgeBandsM[edgeBandIndex(edge, t, level)] ?? NO_STRIDE_BAND;
  }
  const band = (bx: number, by: number): number =>
    blockVertexBandM(scale, clusterErrors, edgeBandsM, bx, by, level);
  const bx = Math.floor(x / 8);
  const by = Math.floor(y / 8);
  const side = (ax: number, ay: number): number => {
    const a = band(ax, ay);
    const b = band(bx, by);
    return a > 0 && b > 0 ? Math.min(a, b) : NO_STRIDE_BAND;
  };
  if (x % 8 === 0) {
    return side(bx - 1, by);
  }
  if (y % 8 === 0) {
    return side(bx, by - 1);
  }
  return band(bx, by);
}

/**
 * Cluster `cluster`'s bands, m, towards strides 2, 4 and 8 (see the module's remarks): the largest
 * band of the vertices it drops, none where any is none. A block's own need, its neighbours' on its
 * sides within the patch, and its patch's edges' shared bands along it; a strip's edge's shared
 * bands along its block (strip m of edge e, anticlockwise in `gpu/resources.ts`' numbering); none
 * for a fin or head run, drawn at stride 1 (`clusterCull.wgsl`'s `cluster_bands`).
 */
export function clusterStrideBandsM(
  scale: StrideScale,
  clusterErrors: ArrayLike<number>,
  edgeBandsM: StrideEdgeBandsM,
  cluster: number,
): StrideBandsM {
  const edgeBands = (edge: number, t: number): StrideBandsM => [
    edgeBandsM[edgeBandIndex(edge, t, 0)] ?? NO_STRIDE_BAND,
    edgeBandsM[edgeBandIndex(edge, t, 1)] ?? NO_STRIDE_BAND,
    edgeBandsM[edgeBandIndex(edge, t, 2)] ?? NO_STRIDE_BAND,
  ];
  if (cluster >= 64 && cluster < 96) {
    const e = Math.floor((cluster - 64) / 8);
    const m = (cluster - 64) % 8;
    // Edge e of `gpu/resources.ts` (0 y = 0, 1 x = 64, 2 y = 64, 3 x = 0), anticlockwise.
    const edge = [2, 1, 3, 0][e] ?? 0;
    return edgeBands(edge, e < 2 ? m : 7 - m);
  }
  if (cluster < 0 || cluster >= 64) {
    return NO_STRIDE_BANDS;
  }
  const bx = cluster % 8;
  const by = Math.floor(cluster / 8);
  const need = (block: number): StrideBandsM => [
    strideNeedM(scale, blockErrorM(clusterErrors, block, 0)),
    strideNeedM(scale, blockErrorM(clusterErrors, block, 1)),
    strideNeedM(scale, blockErrorM(clusterErrors, block, 2)),
  ];
  let bands = need(cluster);
  const sides: ReadonlyArray<readonly [boolean, () => StrideBandsM]> = [
    [bx > 0, () => need(cluster - 1)],
    [bx < 7, () => need(cluster + 1)],
    [by > 0, () => need(cluster - 8)],
    [by < 7, () => need(cluster + 8)],
    [bx === 0, () => edgeBands(0, by)],
    [bx === 7, () => edgeBands(1, by)],
    [by === 0, () => edgeBands(2, bx)],
    [by === 7, () => edgeBands(3, bx)],
  ];
  for (const [on, of] of sides) {
    if (on) {
      bands = sharedStrideBandsM(bands, of());
    }
  }
  return bands;
}

/**
 * The coarsest stride's level a camera pass draws a cluster at, 0 to `capLevel` (`clusterCull.wgsl`'s
 * `camera_stride_level`): the most l such that every band below it is taken, its nearest point
 * `distanceM` at least (1 + β) of the band towards 2^l, all in `f32` as the kernel forms them.
 */
export function cameraStrideLevel(
  bandsM: StrideBandsM,
  capLevel: number,
  distanceM: number,
): number {
  const f = Math.fround;
  const d = f(distanceM);
  let level = 0;
  for (let l = 1; l <= Math.min(capLevel, STRIDE_MORPH_LEVELS); l += 1) {
    const band = f(bandsM[l - 1] ?? NO_STRIDE_BAND);
    if (!(band > 0) || d < f(f(1 + STRIDE_BAND_FRACTION) * band)) {
      break;
    }
    level = l;
  }
  return level;
}

/**
 * The morph level of grid vertex (x, y) of a patch's 65-grid, 0 to 3: the most j at or below 3 for
 * which both are multiples of 2^j (`terrainStrides.wgsl`'s `stride_morph_level`).
 */
export function strideMorphLevel(x: number, y: number): number {
  const bits = (x | y | 8) >>> 0;
  return 31 - Math.clz32(bits & -bits);
}

/**
 * The two neighbours vertex (x, y) of morph level `level` (below 3) moves between: at 2^j along x,
 * along y, or on the quad's (0, 0)–(1, 1) diagonal, as its place's parity at 2^j has it
 * (`stride_parents`).
 */
export function strideParents(
  x: number,
  y: number,
  level: number,
): readonly [ax: number, ay: number, bx: number, by: number] {
  const s = 1 << level;
  const ox = (x >> level) & 1;
  const oy = (y >> level) & 1;
  if (ox === 1 && oy === 1) {
    return [x - s, y - s, x + s, y + s];
  }
  return ox === 1 ? [x - s, y, x + s, y] : [x, y - s, x, y + s];
}

/**
 * The stride morph's k at a vertex of `band`, m, at its own distance `distanceM`: clamp((d − U) ÷
 * (β U), 0, 1), 0 for no band or one past `f32`'s reach, an edge's +∞ (`stride_factor`, before its
 * holds).
 */
export function strideMorphFactor(bandM: number, distanceM: number): number {
  if (!(bandM > 0 && bandM < 3.0e38)) {
    return 0;
  }
  return Math.min(1, Math.max(0, (distanceM - bandM) / (STRIDE_BAND_FRACTION * bandM)));
}

/**
 * How long a coarse–fine hold at k = 0 takes to hold its edge's level-2 vertices once it switches on,
 * ms (a hand value, lean, 2026-11-03): its stride morph falls from its last k to 0 over it, so that
 * a finer patch arriving beside a coarse edge moves the edge's vertices at most 1 ÷ 18 of their
 * stride's displacement a frame at 60 Hz, the two patches meanwhile parting by at most that
 * displacement over the skirts (`StrideHoldFades`).
 */
export const STRIDE_HOLD_FADE_MS = 300;

/**
 * The stride morph's k at grid vertex (x, y) of morph level `level` (below 3) of a patch whose
 * quadrant word is `quadrantWord`, its band `bandM` at its own distance `distanceM`
 * (`terrainStrides.wgsl`'s `stride_factor`, before its contacts' hold): 1 at a level-0 vertex on an
 * edge a coarser patch touches (its coarse–fine hold at k = 1, bits 10 to 13); at a level-2 vertex
 * on an edge a finer one touches (k = 0, bits 2 to 5) its band's k times that edge's byte of
 * `holdFadesWord` over 255, the share of its stride morph the hold's onset still leaves it (0 once
 * the hold is whole); else its band's.
 */
export function strideFactor(
  x: number,
  y: number,
  level: number,
  quadrantWord: number,
  holdFadesWord: number,
  bandM: number,
  distanceM: number,
): number {
  const edges = (x === 0 ? 1 : 0) | (x === 64 ? 2 : 0) | (y === 0 ? 4 : 0) | (y === 64 ? 8 : 0);
  const held = (quadrantWord >>> 2) & edges;
  if (level === 2 && held !== 0) {
    const share = ((holdFadesWord >>> (8 * Math.log2(held & -held))) & 0xff) / 255;
    return share > 0 ? strideMorphFactor(bandM, distanceM) * share : 0;
  }
  if (level === 0 && ((quadrantWord >>> 10) & edges) !== 0) {
    return 1;
  }
  return strideMorphFactor(bandM, distanceM);
}

/**
 * The most vertices {@link stridePlace} places for one, the vertex and every neighbour it moves by:
 * 9, at a level-0 vertex with every level about it morphing (`strides.test.ts` counts them over
 * the grid); the WGSL's arrays' length.
 */
export const STRIDE_NODES = 9;

/**
 * Vertex (x, y)'s place under the stride morph (`terrainStrides.wgsl`'s `stride_place`): its own,
 * from `place`, moved towards the mean of its neighbours' by its k, from `factor`, each neighbour
 * placed so in turn, the coarsest first. `place` gives a vertex's values (its place and whatever
 * else the morph mixes, a fixed length), `factor` its k at its morph level from its own values (0
 * at level 3). The vertices are visited breadth first and held once each, as the WGSL does.
 *
 * @throws RangeError past {@link STRIDE_NODES} vertices, which no 65-grid's vertex needs.
 */
export function stridePlace(
  x: number,
  y: number,
  place: (x: number, y: number) => ReadonlyArray<number>,
  factor: (x: number, y: number, level: number, values: ReadonlyArray<number>) => number,
): number[] {
  const xs = [x];
  const ys = [y];
  const values: number[][] = [];
  const levels: number[] = [];
  const ks: number[] = [];
  const parents: Array<readonly [number, number] | null> = [];
  const indexOf = (px: number, py: number): number => {
    for (let i = 0; i < xs.length; i += 1) {
      if (xs[i] === px && ys[i] === py) {
        return i;
      }
    }
    if (xs.length >= STRIDE_NODES) {
      throw new RangeError(
        `vertex (${String(x)}, ${String(y)}) moves by more than ${String(STRIDE_NODES)}`,
      );
    }
    xs.push(px);
    ys.push(py);
    return xs.length - 1;
  };
  for (let i = 0; i < xs.length; i += 1) {
    const vx = xs[i] ?? 0;
    const vy = ys[i] ?? 0;
    const own = [...place(vx, vy)];
    const level = strideMorphLevel(vx, vy);
    const k = level < STRIDE_MORPH_LEVELS ? factor(vx, vy, level, own) : 0;
    values.push(own);
    levels.push(level);
    ks.push(k);
    if (k > 0) {
      const [ax, ay, bx, by] = strideParents(vx, vy, level);
      parents.push([indexOf(ax, ay), indexOf(bx, by)]);
    } else {
      parents.push(null);
    }
  }
  for (let level = STRIDE_MORPH_LEVELS - 1; level >= 0; level -= 1) {
    for (let i = 0; i < xs.length; i += 1) {
      const of = parents[i];
      const own = values[i];
      const k = ks[i] ?? 0;
      if (levels[i] !== level || of === null || of === undefined || own === undefined) {
        continue;
      }
      const a = values[of[0]] ?? own;
      const b = values[of[1]] ?? own;
      values[i] = own.map((v, n) => v + (((a[n] ?? 0) + (b[n] ?? 0)) * 0.5 - v) * k);
    }
  }
  return values[0] ?? [];
}
