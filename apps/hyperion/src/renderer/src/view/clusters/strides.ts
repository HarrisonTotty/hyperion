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
 *   `effectiveTauPx(τ, limitExcess, w)`, unslewed): at distances of at least A(L, s) = (E_L + e_s) ÷
 *   τ′_rad, τ′_rad = τ′ ÷ (W ÷ (2 tan(fov_h ÷ 2))). The e_s it takes is the largest over the
 *   level's patches the view draws (`BakedPatch.clusterErrors`; {@link StrideLevelErrors}), not
 *   each cluster's own: a shared vertex must be placed alike by every cluster that draws it, which
 *   a threshold of its own a cluster would part (lean, 2026-11-03).
 * - **The ladder.** The thresholds are one ladder a view, U(n) by the absolute level n of the chords
 *   a stride draws: a level-L patch at stride 2^(l + 1) draws the chords of level L − l − 1, and
 *   takes its bands from U(L − 1), U(L − 2) and U(L − 3) ({@link strideBandsOf}). A vertex's place
 *   then depends on where it lies on the body alone, never on which patch draws it, so two patches
 *   of one level, and a coarse and a fine one, place their shared vertices alike (the fine patch's
 *   vertex at 2^(j + 1) of its grid is the coarse one's at 2^j, and both take U(L − j − 1)). Each
 *   U(n) is the largest A(n + l, 2^l) of a resident level (l = 1, 2, 3), and at least the next
 *   finer one's ({@link strideLadder}), so that a level's bands rise with its strides. Two bands of
 *   one place may overlap: a vertex's neighbours may be moving as it moves, which the morph's
 *   evaluation of each neighbour in turn holds exact.
 * - **The stride morph** (CDLOD's distance morph, Strugar 2010, applied to the strides; stateless):
 *   a vertex at morph level j, its grid place a multiple of 2^j but not of 2^(j + 1) (capped at 3,
 *   a block's corners, which never move), is what the stride-2^(j + 1) mesh drops; it moves towards
 *   the mean of its two neighbours at 2^j along its quad's side or on its (0, 0)–(1, 1) diagonal
 *   ({@link strideParents}), the coarser mesh's interpolant there, by k_j = clamp((d − U) ÷ (β U),
 *   0, 1), d its own distance and U its band ({@link STRIDE_BAND_FRACTION}). The neighbours are
 *   placed so themselves, so a vertex's place is a function of the vertex alone
 *   ({@link stridePlace}), and a cluster drawn at stride 2^m, every vertex it drops at k = 1, draws
 *   exactly the morphed surface: the cull takes 2^m only where the cluster's nearest point is past
 *   (1 + β) U of its stride-2^m band ({@link cameraStrideLevel}), so a stride flipping there moves
 *   nothing, at any stride beside any other. The morph mixes the vertex's height, its CDLOD k and
 *   its datum's normal with its place, so that the varyings flip with it.
 * - **Where it holds.** A contact's hold (R05 Design note 6) caps each k at the vertex's hold, so
 *   the ground within every held radius is the finest interpolant (Design note 10); where a finer
 *   patch touches an edge (its coarse–fine hold at k = 0), the edge's level-2 vertices stay, since
 *   the finer patch's vertices there are its block corners; and where a coarser one does (its hold
 *   at k = 1), the edge's level-0 vertices, at the coarser grid's midpoints, move wholly, so that
 *   they stand on its edge wherever its vertices move ({@link strideHeldFactor}).
 */

import { MAX_LEVEL } from "../terrain/patchKey";
import type { PlanetGeometry } from "../terrain/planet";
import { selectionErrorM } from "../terrain/select";

/**
 * β, a stride's band's width over its start: a vertex dropped at stride 2s morphs over the
 * distances U to (1 + β) U, and the cull takes 2s at (1 + β) U (a hand value, lean, 2026-11-03).
 * Well under selection's margin m (`SELECTION_MARGIN`, 0.1): a patch is drawn from at least
 * (1 + m) times the distance at which its level's stated error meets τ′, so its nearest clusters
 * still stride where its e_s is small beside that error; at β = m the seamless probe drew 0.69 of
 * its lit triangles against 0.18 with no band, while a stride's change moved a vertex 0.001 px at
 * most.
 */
export const STRIDE_BAND_FRACTION = 0.02;

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

/** A view's three bands for one level's patches, m: towards strides 2, 4 and 8. */
export type StrideBandsM = readonly [two: number, four: number, eight: number];

/** No strides: every band {@link NO_STRIDE_BAND}. */
export const NO_STRIDE_BANDS: StrideBandsM = [NO_STRIDE_BAND, NO_STRIDE_BAND, NO_STRIDE_BAND];

/** A cluster's or a level's e₂, e₄ and e₈, m. */
export type StrideErrorsM = readonly [two: number, four: number, eight: number];

/** The largest e_s a level holds, m, three a level (s = 2, 4, 8) in {@link StrideLevelErrors}. */
export const STRIDE_LEVEL_ERRORS = 3;

/**
 * A frame's decay of a view's levels' errors (see {@link StrideLevelErrors}), as a fraction (a hand
 * value, lean, 2026-11-03): a level's bands come back in by at most a tenth of their width
 * ({@link STRIDE_BAND_FRACTION}) a frame once its roughest patch leaves the view, so that a vertex's
 * k moves by at most 0.1 a frame for it, and the bands halve in some 350 frames.
 */
export const STRIDE_ERROR_DECAY = 0.002;

/**
 * The largest measured e₂, e₄ and e₈ over each level's clusters, m, for levels 0 to `MAX_LEVEL`, as
 * a view's ladder takes them (lean, 2026-11-03): raised at once to its drawn patches' each frame,
 * which the drawn error needs, and else lowered by at most {@link STRIDE_ERROR_DECAY} a frame; a
 * level none has held reads −1.
 */
export class StrideLevelErrors {
  /** Three a level, e₂, e₄, e₈, m; −1 for a level with none. */
  readonly errorsM = new Float64Array((MAX_LEVEL + 1) * STRIDE_LEVEL_ERRORS).fill(-1);

  /** Raises `level`'s errors to a bake's largest ({@link largestStrideErrorsM} of its clusters). */
  raise(level: number, largestM: StrideErrorsM): void {
    const base = STRIDE_LEVEL_ERRORS * level;
    for (let l = 0; l < STRIDE_LEVEL_ERRORS; l += 1) {
      const at = base + l;
      this.errorsM[at] = Math.max(this.errorsM[at] ?? -1, largestM[l] ?? Infinity);
    }
  }

  /** Raises `level`'s errors by a bake's clusters' errors (`BakedPatch.clusterErrors`). */
  take(level: number, clusterErrors: Float16Array): void {
    this.raise(level, largestStrideErrorsM(clusterErrors));
  }

  /** Lowers every level's errors by {@link STRIDE_ERROR_DECAY}, as a frame begins. */
  decay(): void {
    for (let at = 0; at < this.errorsM.length; at += 1) {
      const e = this.errorsM[at] ?? -1;
      if (e > 0 && Number.isFinite(e)) {
        this.errorsM[at] = e * (1 - STRIDE_ERROR_DECAY);
      }
    }
  }

  /** Forgets every level's errors, as for a new device. */
  clear(): void {
    this.errorsM.fill(-1);
  }
}

/**
 * The largest e₂, e₄ and e₈ of a bake's clusters, m (`BakedPatch.clusterErrors`, four half floats a
 * cluster, e₁ first), +∞ for a NaN.
 */
export function largestStrideErrorsM(clusterErrors: Float16Array): StrideErrorsM {
  const most = [0, 0, 0];
  for (let c = 0; 4 * c + 3 < clusterErrors.length; c += 1) {
    for (let l = 0; l < STRIDE_LEVEL_ERRORS; l += 1) {
      const e = clusterErrors[4 * c + l + 1] ?? Number.POSITIVE_INFINITY;
      most[l] = Math.max(most[l] ?? 0, Number.isNaN(e) ? Infinity : e);
    }
  }
  return [most[0] ?? Infinity, most[1] ?? Infinity, most[2] ?? Infinity];
}

/**
 * A view's ladder (see the module's remarks): U(n) for n from 0 to `MAX_LEVEL` into `outM`, m, the
 * band at which the chords of level n begin to be drawn, {@link NO_STRIDE_BAND} where no resident
 * level takes it or the view takes no strides.
 *
 * @param toleranceRad - τ′ over the view's px per radian; at or below 0, or not finite, no strides.
 * @param levelErrorsM - {@link StrideLevelErrors.errorsM}.
 */
export function strideLadder(
  planet: PlanetGeometry,
  toleranceRad: number,
  levelErrorsM: Float64Array,
  outM: Float64Array,
): Float64Array {
  outM.fill(NO_STRIDE_BAND);
  if (!(toleranceRad > 0 && Number.isFinite(toleranceRad))) {
    return outM;
  }
  const deepest = Math.min(planet.finestLevel, MAX_LEVEL);
  let finer = NO_STRIDE_BAND;
  for (let n = deepest - 1; n >= 0; n -= 1) {
    let need = Number.NEGATIVE_INFINITY;
    for (let l = 1; l <= STRIDE_MORPH_LEVELS; l += 1) {
      const level = n + l;
      const e = level <= deepest ? (levelErrorsM[STRIDE_LEVEL_ERRORS * level + l - 1] ?? -1) : -1;
      if (e >= 0) {
        need = Math.max(need, (selectionErrorM(planet, level) + e) / toleranceRad);
      }
    }
    if (need === Number.NEGATIVE_INFINITY) {
      finer = NO_STRIDE_BAND;
      continue;
    }
    need = Math.max(need, finer);
    const band = Number.isFinite(need) ? need : NO_STRIDE_BAND;
    outM[n] = band;
    finer = band;
  }
  return outM;
}

/**
 * A level-`level` patch's bands from `ladder`, m: U(L − 1), U(L − 2) and U(L − 3), each
 * {@link NO_STRIDE_BAND} where the ladder has none or L − l − 1 is below 0.
 */
export function strideBandsOf(ladderM: Float64Array, level: number): StrideBandsM {
  const band = (n: number): number => (n >= 0 ? (ladderM[n] ?? NO_STRIDE_BAND) : NO_STRIDE_BAND);
  return [band(level - 1), band(level - 2), band(level - 3)];
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
 * (β U), 0, 1), 0 for no band (`stride_factor`, before its holds).
 */
export function strideMorphFactor(bandM: number, distanceM: number): number {
  if (!(bandM > 0)) {
    return 0;
  }
  return Math.min(1, Math.max(0, (distanceM - bandM) / (STRIDE_BAND_FRACTION * bandM)));
}

/**
 * The stride morph's hold at grid vertex (x, y) of morph level `level` of a patch whose quadrant word
 * is `quadrantWord` (`terrainStrides.wgsl`'s `stride_factor`): 0 at a level-2 vertex on an edge a
 * finer patch touches (its coarse–fine hold at k = 0, bits 2 to 5), 1 at a level-0 vertex on an
 * edge a coarser one does (k = 1, bits 10 to 13), else `null`, its k its band's.
 */
export function strideHeldFactor(
  x: number,
  y: number,
  level: number,
  quadrantWord: number,
): number | null {
  const edges = (x === 0 ? 1 : 0) | (x === 64 ? 2 : 0) | (y === 0 ? 4 : 0) | (y === 64 ? 8 : 0);
  if (level === 2 && ((quadrantWord >>> 2) & edges) !== 0) {
    return 0;
  }
  if (level === 0 && ((quadrantWord >>> 10) & edges) !== 0) {
    return 1;
  }
  return null;
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
