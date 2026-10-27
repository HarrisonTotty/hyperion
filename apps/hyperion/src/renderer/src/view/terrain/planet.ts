/**
 * The planet's geometry as selection reads it: its reference spheroid and the level table the
 * height workers' module hands over at run time (plan R05, T7.a, Design notes 5, 7 and 15), with
 * the body's σ by level and the bound selection charges each level under the rule in force (plan
 * R10, R10.T4.b; `levelBounds.ts`).
 */

import type { Vec3 } from "../../geometry/vec3";
import { BAND_LIMIT_M, finestLevel, type Xyz } from "./cube";
import { checkLevelSigma, SELECTION_RULE, selectionBound, type SelectionRule } from "./levelBounds";
import { MAX_LEVEL } from "./patchKey";

/**
 * A vector in a body's rotating, body-fixed axes (z along the pole), in metres unless its name says
 * otherwise; positions reach R02 as a `ViewPosition` of kind `body_fixed`.
 */
export type BodyFixedVec3 = Vec3;

/**
 * A body's reference spheroid, the datum heights are measured from (Design note 5): equatorial
 * radius a and polar radius c, metres, about the body-fixed z axis; a sphere is a = c.
 *
 * @remarks
 * The same shape as R07's `BodyFigure`, which R07 re-exports from here. `pole` is the rotation
 * axis's direction for R07, in its frame there; `null` where it is not known. Selection does not
 * read it: in the body-fixed axes the pole is +z.
 */
export interface BodyFigure {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
  readonly pole: Vec3 | null;
}

/** The planet's geometry for selection and bounds. */
export interface PlanetGeometry {
  readonly figure: BodyFigure;
  /** The deepest level drawn, from the equatorial radius (Design note 3; 19 for an Earth). */
  readonly finestLevel: number;
  /**
   * The module's `levelTable()`: for level n from 0 to {@link MAX_LEVEL}, at index 4n, the level
   * bound ε_n, the lowest and the highest height, and the largest vertex spacing, metres; `null`
   * for R07's zero-height `mesh` regime, a reference spheroid with no height worker.
   */
  readonly levels: Float64Array | null;
  /**
   * The body's σ_n by level, 25, metres: the workers' `levelSigmaM()` (R10.T4.b); `null` where none
   * was given, as for the test planet's table alone and R07's reference spheroid.
   */
  readonly sigmaM: Float64Array | null;
  /**
   * The bound selection charges each level, 25, metres: {@link levels}' ε_n under
   * {@link rule} with {@link sigmaM} (`levelBounds.ts`' `selectionBound`); `null` with no table.
   * Culling, the skirts and the baked ranges' widening read the stated ε_n instead.
   */
  readonly selectionBoundM: Float64Array | null;
  /** The rule {@link selectionBoundM} is made under: `levelBounds.ts`' rule in force unless given. */
  readonly rule: SelectionRule;
}

/** Entries a level table holds per level. */
export const LEVEL_TABLE_STRIDE = 4;

/**
 * The planet's geometry from its figure and the module's level table, or a `null` table for a
 * zero-height reference spheroid (R07 Design notes 3 and 19), with the body's σ by level where
 * the workers gave it and the selection rule (R10.T4.b).
 *
 * @param sigmaM - The body's σ_n by level, 25 (`levelBounds.ts`' `LevelBounds.sigmaM`); `null`,
 *   or with no table, the selection bound is the stated one under either rule.
 * @param rule - The rule the selection bound is made under; the rule in force unless a test or a
 *   count asks for another.
 * @throws Error if the figure's radii are not finite and positive with c ≤ a, the table is not
 *   {@link LEVEL_TABLE_STRIDE} × 25 finite numbers, or σ is not 25 finite values of at least 0.
 */
export function planetGeometry(
  figure: BodyFigure,
  levelTable: Float64Array | null,
  sigmaM: Float64Array | null = null,
  rule: SelectionRule = SELECTION_RULE,
): PlanetGeometry {
  const { equatorialRadiusM: a, polarRadiusM: c } = figure;
  if (!(Number.isFinite(a) && Number.isFinite(c) && c > 0 && c <= a)) {
    throw new Error(`a body's figure needs 0 < c ≤ a, got a = ${a} m, c = ${c} m`);
  }
  if (levelTable !== null) {
    const expected = LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1);
    if (levelTable.length !== expected || !levelTable.every((x) => Number.isFinite(x))) {
      throw new Error(`a level table has ${expected} finite entries, got ${levelTable.length}`);
    }
  }
  if (sigmaM !== null) {
    checkLevelSigma(sigmaM);
  }
  return {
    figure,
    finestLevel: finestLevel(a),
    levels: levelTable,
    sigmaM,
    selectionBoundM: levelTable === null ? null : selectionBounds(levelTable, sigmaM, rule),
    rule,
  };
}

/** Each level's selection bound from the table's stated ε_n and σ_n under `rule`. */
function selectionBounds(
  levelTable: Float64Array,
  sigmaM: Float64Array | null,
  rule: SelectionRule,
): Float64Array {
  const bounds = new Float64Array(MAX_LEVEL + 1);
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const hardM = levelTable[LEVEL_TABLE_STRIDE * level] ?? 0;
    bounds[level] = selectionBound(hardM, sigmaM?.[level] ?? 0, rule);
  }
  return bounds;
}

function entry(planet: PlanetGeometry, level: number, offset: number): number {
  if (planet.levels === null) {
    return 0;
  }
  const value = planet.levels[LEVEL_TABLE_STRIDE * level + offset];
  if (value === undefined) {
    throw new Error(`level ${level} is outside the level table`);
  }
  return value;
}

/**
 * The level bound ε_n, metres: the most a level's mesh can depart from the finest level's
 * (Design note 15), zero with no table (a smooth spheroid has no relief to omit).
 */
export function levelBoundM(planet: PlanetGeometry, level: number): number {
  return entry(planet, level, 0);
}

/**
 * The bound selection charges `level`, metres: the stated ε_n ({@link levelBoundM}) under the hard
 * rule, min(ε_n, 4σ_n) under the statistical one ({@link PlanetGeometry.selectionBoundM}; R10.T4.b),
 * zero with no table. Only the error selection charges reads it; every bounding volume keeps ε_n.
 *
 * @throws Error if `level` is outside the level table.
 */
export function selectionBoundM(planet: PlanetGeometry, level: number): number {
  if (planet.selectionBoundM === null) {
    return 0;
  }
  const value = planet.selectionBoundM[level];
  if (value === undefined) {
    throw new Error(`level ${level} is outside the level table`);
  }
  return value;
}

/** The lowest and highest heights a patch of `level` can reach above the datum, metres. */
export function levelHeightRangeM(
  planet: PlanetGeometry,
  level: number,
): readonly [number, number] {
  return [entry(planet, level, 1), entry(planet, level, 2)];
}

/**
 * One `f32` step's relative size: a height rounded to `f32` is within this of its value, as the
 * bake's `f32_step` takes it for the skirts' depth.
 */
export const F32_RELATIVE_STEP = 2 ** -23;

/**
 * Per level, the lowest bottom and the highest top any patch of the level, or of a finer level
 * beneath one, can reach, before the skirt margin (see {@link subtreeHeightRangeM}).
 */
interface SubtreeRanges {
  readonly lowM: Float64Array;
  readonly highM: Float64Array;
}

const subtreeRanges = new WeakMap<PlanetGeometry, SubtreeRanges>();

/** The planet's {@link SubtreeRanges}, made on first use (the planet is immutable). */
function subtreeRangesOf(planet: PlanetGeometry): SubtreeRanges {
  let ranges = subtreeRanges.get(planet);
  if (ranges === undefined) {
    const rows = MAX_LEVEL + 1;
    // From each level to the deepest drawn, the planet's finest: the lowest skirt bottom and the
    // highest height. No patch beneath the finest level is drawn, so from it on each level is its
    // own.
    const fromLow = new Float64Array(rows);
    const fromHigh = new Float64Array(rows);
    let low = Infinity;
    let high = -Infinity;
    for (let m = MAX_LEVEL; m >= 0; m -= 1) {
      if (m >= planet.finestLevel) {
        low = Infinity;
        high = -Infinity;
      }
      const [rowLow, rowHigh] = levelHeightRangeM(planet, m);
      // A level-m bake hangs its skirts from its own heights and its morph targets, its parent
      // level's, alike: by ε_m, an f32 step of the largest height over both and its margin.
      const [parentLow, parentHigh] = m > 0 ? levelHeightRangeM(planet, m - 1) : [rowLow, rowHigh];
      const largest = Math.max(
        Math.abs(rowLow),
        Math.abs(rowHigh),
        Math.abs(parentLow),
        Math.abs(parentHigh),
      );
      const bottom =
        Math.min(rowLow, parentLow) - levelBoundM(planet, m) - largest * F32_RELATIVE_STEP;
      low = Math.min(low, bottom);
      high = Math.max(high, rowHigh);
      fromLow[m] = low;
      fromHigh[m] = high;
    }
    // Level n's subtree takes the rows from its parent's on: a patch's morph targets are its
    // parent level's heights.
    const lowM = new Float64Array(rows);
    const highM = new Float64Array(rows);
    for (let n = 0; n < rows; n += 1) {
      const from = Math.max(0, n - 1);
      lowM[n] = fromLow[from] ?? 0;
      highM[n] = fromHigh[from] ?? 0;
    }
    ranges = { lowM, highM };
    subtreeRanges.set(planet, ranges);
  }
  return ranges;
}

/**
 * The lowest and highest heights the patches of `level`, and of every finer level beneath one, can
 * reach above the datum, metres, skirts included: over the level table's rows from the level's
 * parent to the deepest drawn ({@link PlanetGeometry.finestLevel}), each level's skirt bottom the lower of its row's low and its parent row's,
 * its morph targets', less its skirt depth (ε_m, an `f32` step of the largest |h| over both rows
 * and `skirtMarginM`, as the bake hangs its skirts). Selection drops a patch's
 * whole subtree when no view sees its box, so a box it prunes by holds every descendant's
 * vertices, morph targets and skirts (R09.T8.d; decision-r09-t8d-reach, follow-up 1): a coarse
 * level's own row may hold far less than a finer one's.
 *
 * @throws Error if `level` is outside the level table.
 */
export function subtreeHeightRangeM(
  planet: PlanetGeometry,
  level: number,
  skirtMarginM = 0,
): readonly [number, number] {
  const ranges = subtreeRangesOf(planet);
  const low = ranges.lowM[level];
  const high = ranges.highM[level];
  if (!Number.isInteger(level) || low === undefined || high === undefined) {
    throw new Error(`level ${level} is outside the level table`);
  }
  return [low - skirtMarginM, high];
}

/** The lowest height anywhere on the planet, metres: the occluder's depth below the datum. */
export function lowestHeightM(planet: PlanetGeometry): number {
  let lowest = 0;
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    lowest = Math.min(lowest, levelHeightRangeM(planet, level)[0]);
  }
  return lowest;
}

/**
 * The terrain's band limit, metres: the wasm module's `bandLimitM()`, which the render thread
 * cannot call (only workers load the module), mirrored here and checked equal to the module's by
 * `planet.wasm.test.ts`. R11 reads it (Design note 5).
 */
export function bandLimitM(): number {
  return BAND_LIMIT_M;
}

/** The spheroid point M·d of the unit direction `dir`, body-fixed metres. */
export function spheroidPoint(figure: BodyFigure, dir: Xyz): Xyz {
  return [
    figure.equatorialRadiusM * dir[0],
    figure.equatorialRadiusM * dir[1],
    figure.polarRadiusM * dir[2],
  ];
}

/** The outward unit normal ν = M⁻¹d ÷ |M⁻¹d| at the spheroid point of the unit direction `dir`. */
export function spheroidNormal(figure: BodyFigure, dir: Xyz): Xyz {
  const m: Xyz = [
    dir[0] / figure.equatorialRadiusM,
    dir[1] / figure.equatorialRadiusM,
    dir[2] / figure.polarRadiusM,
  ];
  const len = Math.sqrt(m[0] * m[0] + m[1] * m[1] + m[2] * m[2]);
  return [m[0] / len, m[1] / len, m[2] / len];
}

/** The point `heightM` metres above the spheroid along its normal over `dir`: M·d + h·ν. */
export function surfacePoint(figure: BodyFigure, dir: Xyz, heightM: number): Xyz {
  const p = spheroidPoint(figure, dir);
  const nu = spheroidNormal(figure, dir);
  return [p[0] + heightM * nu[0], p[1] + heightM * nu[1], p[2] + heightM * nu[2]];
}
