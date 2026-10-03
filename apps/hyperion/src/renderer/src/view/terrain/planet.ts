/**
 * The planet's geometry as selection reads it: its reference spheroid and the level table the
 * height workers' module hands over at run time (plan R05, T7.a, Design notes 5, 7 and 15).
 */

import type { Vec3 } from "../../geometry/vec3";
import { BAND_LIMIT_M, finestLevel, type Xyz } from "./cube";
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
}

/** Entries a level table holds per level. */
export const LEVEL_TABLE_STRIDE = 4;

/**
 * The planet's geometry from its figure and the module's level table, or a `null` table for a
 * zero-height reference spheroid (R07 Design notes 3 and 19).
 *
 * @throws Error if the figure's radii are not finite and positive with c ≤ a, or the table is not
 *   {@link LEVEL_TABLE_STRIDE} × 25 finite numbers.
 */
export function planetGeometry(
  figure: BodyFigure,
  levelTable: Float64Array | null,
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
  return { figure, finestLevel: finestLevel(a), levels: levelTable };
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

/** The lowest and highest heights a patch of `level` can reach above the datum, metres. */
export function levelHeightRangeM(
  planet: PlanetGeometry,
  level: number,
): readonly [number, number] {
  return [entry(planet, level, 1), entry(planet, level, 2)];
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
