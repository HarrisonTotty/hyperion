/**
 * The descent's predicted patch demand (plan R05, T13.a, Design note 19): the brainstorm's closed
 * form at a fixed k, and the per-level prediction from T6's level table that the runs are measured
 * against; and the level table under min(hard, 4σ_n) that the patch-demand ruling (2026-10-03) has
 * T13.a record beside the hard one.
 *
 * @remarks
 * A level of patch size S drawn from k S to 2k S exposes 4k patches along its leading edge, so
 * horizontal motion at v demands Σ_L 4 k_L v ÷ S_L a second over the levels drawn beyond the
 * altitude; each halving of altitude re-bakes the new finest level's nadir disc, 3π k² patches,
 * so a vertical speed ḣ demands (3π k² ÷ ln 2) |ḣ| ÷ h. At a fixed k = 5 the two are the
 * brainstorm's (200 v + 290 |ḣ|) ÷ h with the vertical constant re-derived as 340. The ratio
 * k_L = ε_L ÷ (S_L τ θ_px) comes from the level bound, with θ_px = 2 tan(fov ÷ 2) ÷ W_px, and h is
 * floored at the cap k S_finest, below which the finest level is drawn under the camera whatever
 * the altitude.
 */

import {
  LEVEL_TABLE_STRIDE,
  levelBoundM,
  type PlanetGeometry,
  planetGeometry,
} from "../terrain/planet";

/** The closed form's fixed ratio of distance to patch size (the brainstorm's k = 5). */
export const CLOSED_FORM_K = 5;

/** The horizontal constant: 8k², 200 at k = 5. */
export function horizontalConstant(k: number): number {
  return 8 * k * k;
}

/** The vertical constant: 3π k² ÷ ln 2, about 340 at k = 5 (the brainstorm's 290 re-derived). */
export function verticalConstant(k: number): number {
  return (3 * Math.PI * k * k) / Math.LN2;
}

/** The camera's motion the prediction reads. */
export interface DemandState {
  /** Altitude above the spheroid, metres. */
  readonly altitudeM: number;
  /** Horizontal and vertical speed, m/s. */
  readonly horizontalSpeedMps: number;
  readonly verticalSpeedMps: number;
}

/**
 * The brainstorm's closed form at a fixed k, patches a second: (8k² v + (3π k² ÷ ln 2) |ḣ|) ÷ h,
 * with h floored at `capM`.
 */
export function closedFormDemandPerS(state: DemandState, capM: number, k = CLOSED_FORM_K): number {
  const h = Math.max(state.altitudeM, capM);
  return (
    (horizontalConstant(k) * state.horizontalSpeedMps +
      verticalConstant(k) * Math.abs(state.verticalSpeedMps)) /
    h
  );
}

/** A view as the prediction reads it. */
export interface DemandView {
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
  /** The presented width, device pixels. */
  readonly widthPx: number;
  /** The tolerance τ, pixels. */
  readonly tauPx: number;
}

/** A patch's side at `level`, metres: 64 of the level's largest vertex spacing. */
export function patchSizeM(planet: PlanetGeometry, level: number): number {
  const levels = planet.levels;
  const spacing = levels?.[LEVEL_TABLE_STRIDE * level + 3];
  if (spacing === undefined) {
    throw new Error(`level ${level} is outside the planet's level table`);
  }
  return 64 * spacing;
}

/** k_L = ε_L ÷ (S_L τ θ_px): the distance, in patch sizes, at which level L meets τ. */
export function levelRatio(planet: PlanetGeometry, level: number, view: DemandView): number {
  const thetaPx = (2 * Math.tan(view.fovXRad / 2)) / view.widthPx;
  return levelBoundM(planet, level) / (patchSizeM(planet, level) * view.tauPx * thetaPx);
}

/**
 * The altitude below which the finest level is drawn under the camera, metres: k S_finest, with k
 * the level above the finest's (the finest's own bound is zero).
 */
export function capAltitudeM(planet: PlanetGeometry, view: DemandView): number {
  const finest = planet.finestLevel;
  return levelRatio(planet, finest - 1, view) * patchSizeM(planet, finest);
}

/** The per-level prediction at one moment, and its parts. */
export interface LevelDemand {
  /** Patches a second in all. */
  readonly perS: number;
  /** The horizontal part, by level (0 where the level is not drawn beyond the altitude). */
  readonly horizontalPerS: ReadonlyArray<number>;
  /** The vertical part. */
  readonly verticalPerS: number;
  /** The altitude used, floored at the cap, metres. */
  readonly floorAltitudeM: number;
}

/**
 * Design note 19's per-level prediction: Σ_L 4 k_L v ÷ S_L over the levels drawn beyond the
 * altitude (k_L S_L ≥ h), the finest level's at k_{f−1} under the cap, plus (3π k² ÷ ln 2) |ḣ| ÷ h
 * with k the deepest such level's.
 */
export function perLevelDemand(
  planet: PlanetGeometry,
  state: DemandState,
  view: DemandView,
): LevelDemand {
  const finest = planet.finestLevel;
  const cap = capAltitudeM(planet, view);
  const h = Math.max(state.altitudeM, cap);
  const horizontalPerS: number[] = [];
  let deepestK = 0;
  for (let level = 0; level <= finest; level += 1) {
    const k =
      level < finest ? levelRatio(planet, level, view) : levelRatio(planet, finest - 1, view);
    const size = patchSizeM(planet, level);
    if (k * size >= h) {
      horizontalPerS.push((4 * k * state.horizontalSpeedMps) / size);
      deepestK = k;
    } else {
      horizontalPerS.push(0);
    }
  }
  const verticalPerS = (verticalConstant(deepestK) * Math.abs(state.verticalSpeedMps)) / h;
  const horizontal = horizontalPerS.reduce((sum, x) => sum + x, 0);
  return { perS: horizontal + verticalPerS, horizontalPerS, verticalPerS, floorAltitudeM: h };
}

/** The rule a level table's bound takes. */
export type BoundRule = "hard" | "calibrated";

/**
 * The planet with each level's bound under `rule`: the hard ε_n, or min(ε_n, 4σ_n) where σ_n > 0
 * (keeping the hard bound where a level omits no octave, as the patch-demand ruling's 4e has it).
 * The one place the 4σ pass's level table is built.
 *
 * @param omittedSigmaM - σ_n by level, the RMS of the octaves level n omits, metres: the module's
 *   `omittedSigmaM(level, ridges)` for the same ridges as the table, which only a worker or a test
 *   can call, so the caller passes it in.
 */
export function boundedPlanet(
  planet: PlanetGeometry,
  rule: BoundRule,
  omittedSigmaM: ReadonlyArray<number>,
): PlanetGeometry {
  if (rule === "hard" || planet.levels === null) {
    return planet;
  }
  return planetGeometry(planet.figure, calibratedLevelTable(planet.levels, omittedSigmaM));
}

/** A level table with its bound column under min(ε_n, 4σ_n), heights and spacing unchanged. */
export function calibratedLevelTable(
  levels: Float64Array,
  omittedSigmaM: ReadonlyArray<number>,
): Float64Array {
  const table = Float64Array.from(levels);
  for (let level = 0; level * LEVEL_TABLE_STRIDE < table.length; level += 1) {
    const sigma = omittedSigmaM[level] ?? 0;
    const index = LEVEL_TABLE_STRIDE * level;
    const hard = table[index] ?? 0;
    table[index] = sigma > 0 ? Math.min(hard, 4 * sigma) : hard;
  }
  return table;
}
