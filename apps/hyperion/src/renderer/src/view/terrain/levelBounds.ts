/**
 * A body's level bounds and the rule selection charges them by (plan R10, R10.T4.b; R10 Design
 * note 5; `signoff-12.md`, item 30).
 *
 * @remarks
 * The height workers answer every block of a body's field with its level bounds: the level table,
 * whose bound column is R09's stated bound ε_n (`level_bound_m`, hard in the structural octaves and
 * the channels, a 1 − 10⁻⁹ per-point quantile in the craters' and rims' terms), and σ_n, the
 * largest of R09's level-keyed `omitted_rms` at the level over the cells the field holds as
 * surveyed (`crates/hyperion-surface/src/worker.rs`' `level_sigma_m`). The pool keeps the latest of
 * each body ({@link LevelBounds}; `HeightWorkerPool.fieldBounds`), and `planetGeometry` takes them
 * for selection.
 *
 * Selection charges a level {@link selectionBound}'s ε_n, plus R05's chord sagitta
 * (`selectionErrorM`), at each view's own τ: the high setting's 1 px, the low setting's 2 px and
 * the wireframe's 4 px alike. Under the hard rule that is the table's ε_n; under the statistical
 * rule it is min(ε_n, 4σ_n), Design note 5's candidate, R05 Design note 15's "maximum error within
 * the chunk" taken statistically. Only the error moves: culling's boxes, the skirts and the baked
 * ranges' widening keep the hard bound (`levelBoundM`), which stays the stated and tested bound.
 *
 * {@link SELECTION_RULE} is the rule in force, the one place the switch is made. Item 30 adopts
 * the statistical rule only where, over R05's scripted descent at τ = 1 px, it selects at most two
 * thirds of the hard rule's patches on every sample world and T4.a's per-patch maxima pass 4σ in
 * at most one patch in three; T4.a's record fails the second on four of the six worlds, so the
 * hard bound stands (`levelBounds.test.ts` applies the rule to the fixture; R10.T4.b's record).
 * decision-r10-item30 keeps it at RM5: at the high setting's budget its mean drawn error is 1.1 to
 * 1.3 times a perfect level-wise bound's (`levelBounds.test.ts`' guard holds it to 1.5), and its
 * 38–178 px τ′ is the stated error, not the drawn one.
 *
 * The rule also sets whether the workers survey σ at all ({@link sigmaSurveyOf}, R10.T4.c): under
 * the hard rule nothing reads it, and they answer 0 at every level, which the statistical rule
 * would charge ε_n for too.
 */

import { MAX_LEVEL, unreachable } from "./patchKey";
import type { SigmaSurveySetting } from "./workers/messages";

/** How selection charges a level: the stated bound, or min(ε_n, 4σ_n). */
export type SelectionRule = "hard" | "statistical";

/**
 * The rule in force: the hard bound, since `signoff-12.md` item 30's second condition fails on
 * T4.a's figures (R10.T4.b's record). Change it here alone.
 */
export const SELECTION_RULE: SelectionRule = "hard";

/**
 * Whether the height workers survey a body's σ by level under `rule`, each `field_init`'s flag
 * (decision-r10-item30 §4): only the statistical rule reads σ, and the survey costs some 25 s of
 * every worker's time for a whole level-8 field (R10.T4.b's record). A switch of rule starts the
 * fields again with it.
 */
export function sigmaSurveyOf(rule: SelectionRule): SigmaSurveySetting {
  switch (rule) {
    case "hard":
      return "skipped";
    case "statistical":
      return "surveyed";
  }
  return unreachable(rule);
}

/** The multiple of σ_n the statistical rule charges: Design note 5's 4σ. */
export const SIGMA_MULTIPLE = 4;

/** The levels a body's σ holds, 0 to {@link MAX_LEVEL}: 25. */
export const LEVEL_SIGMA_LENGTH = MAX_LEVEL + 1;

/** A body's level bounds as its height workers last answered them. */
export interface LevelBounds {
  /**
   * The level table, 25 × 4, level n at 4n: the stated bound ε_n, the lowest and the highest height
   * and the largest vertex spacing, metres (`PlanetGeometry.levels`' layout).
   */
  readonly table: Float64Array;
  /**
   * σ_n by level, 25, level n at n, metres: 0 where the level omits nothing, nothing is surveyed,
   * or the workers skip σ ({@link sigmaSurveyOf}).
   */
  readonly sigmaM: Float64Array;
}

/**
 * The bound selection charges a level of stated bound `hardM` and σ `sigmaM`, metres: `hardM`
 * under the hard rule; under the statistical rule min(`hardM`, 4 `sigmaM`) where σ is positive,
 * and `hardM` where it is 0 (a level that omits nothing, or ground with no σ read), as the
 * patch-demand ruling's 4e keeps it.
 */
export function selectionBound(
  hardM: number,
  sigmaM: number,
  rule: SelectionRule = SELECTION_RULE,
): number {
  switch (rule) {
    case "hard":
      return hardM;
    case "statistical":
      return sigmaM > 0 ? Math.min(hardM, SIGMA_MULTIPLE * sigmaM) : hardM;
  }
  return unreachable(rule);
}

/**
 * Checks a body's σ by level: {@link LEVEL_SIGMA_LENGTH} finite values, none negative.
 *
 * @throws Error if it is not.
 */
export function checkLevelSigma(sigmaM: Float64Array): void {
  if (sigmaM.length !== LEVEL_SIGMA_LENGTH) {
    throw new Error(
      `a body's σ by level has ${String(LEVEL_SIGMA_LENGTH)} values, got ${String(sigmaM.length)}`,
    );
  }
  const level = sigmaM.findIndex((x) => !(Number.isFinite(x) && x >= 0));
  if (level >= 0) {
    throw new Error(
      `a body's σ by level is finite and at least 0, got ${String(sigmaM[level])} m at level ${String(level)}`,
    );
  }
}
