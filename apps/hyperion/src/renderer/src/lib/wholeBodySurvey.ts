/**
 * A whole body's survey: one R09 `survey_pass` over every cell of the body's coarse field, the
 * server's own survey path and R09's stand-in for the sensors (plan R10, R10.T16.d;
 * decision-r10-live-path, gap G3), shared by `just place-ship --survey` and `VIEW`'s body
 * inspection (the owner's request, 2026-11-01), so that a served world's field holds surveyed
 * ground to draw.
 *
 * @remarks
 * The pass covers one run of every cell at the level the server finds from the body's figure
 * ({@link figureLevel}), at one instant. The server records a live pass, `--survey`'s, in the
 * universe's survey log, which keeps it, and one marked for training, the inspection's, in its
 * scratch log of training alone (the owner's delegated call, 2026-11-01); either way it raises the
 * body's coverage revision there, which its answer gives. The module is pure and imports types
 * alone, so that the tool's project builds it too (`tsconfig.node.json`).
 */

import type {
  BodyIdHex,
  RequestOf,
  SurveyScopeDto,
  SurveySourceDto,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";

/**
 * The widest mean cell of a body's coarse field, m: R09's `MAX_MEAN_CELL_M`, the brainstorm's
 * "no wider than about 40 km" (R09 Design note 4). A test holds it to `field.rs`.
 */
export const MAX_MEAN_CELL_M = 40_000;

/** The levels a coarse field takes, R09's `CoarseLevel::MIN` and `MAX`; a test holds them. */
export const COARSE_LEVELS = { min: 5, max: 8 } as const;

/**
 * The level of the coarse field of a body of volumetric mean radius `radiusM`, m: R09's
 * `coarse_level` (`crates/hyperion-surface/src/field.rs`), the shallowest of levels 5 to 7 whose
 * mean cell, (R × √(4π ÷ 6)) ÷ 2ᴸ in that order, is no wider than {@link MAX_MEAN_CELL_M}, and 8
 * otherwise: 6 for the Moon, 8 for an Earth.
 *
 * @throws RangeError for a radius that is not finite and positive.
 */
export function coarseLevel(radiusM: number): number {
  if (!(Number.isFinite(radiusM) && radiusM > 0)) {
    throw new RangeError(`a body's radius must be finite and positive, got ${String(radiusM)} m`);
  }
  const wholeFace = radiusM * Math.sqrt((4 * Math.PI) / 6);
  for (let level: number = COARSE_LEVELS.min; level < COARSE_LEVELS.max; level += 1) {
    if (wholeFace / 2 ** level <= MAX_MEAN_CELL_M) {
      return level;
    }
  }
  return COARSE_LEVELS.max;
}

/** The number of cells of a coarse field at `level`, 6 · 4ᴸ. */
export function coarseCellCount(level: number): number {
  return 6 * 4 ** level;
}

/** A body's figure as its level is found from: its reference spheroid's two radii. */
export interface SurveyedFigure {
  /** a, m. */
  readonly equatorialRadiusM: number;
  /** c, m. */
  readonly polarRadiusM: number;
}

/**
 * The level of the coarse field of a body of `figure`, as the server finds it for a pass:
 * {@link coarseLevel} of the volumetric radius ∛(a² c).
 *
 * @remarks
 * The server takes the radius through its own `cbrt`; `Math.cbrt` may differ from it in the last
 * place, which moves the level only for a radius within an ulp of one of the three radii where it
 * changes.
 *
 * @throws RangeError for a figure whose radii are not finite and positive.
 */
export function figureLevel(figure: SurveyedFigure): number {
  const { equatorialRadiusM: a, polarRadiusM: c } = figure;
  return coarseLevel(Math.cbrt(a * a * c));
}

/**
 * A survey pass to record over a body, R09's stand-in for the sensors (plan R10, R10.T16.d; R09's
 * Risks, "The survey stand-in"): how it measured and how finely.
 */
export interface SurveyAsked {
  readonly source: SurveySourceDto;
  /** How finely it measured, m: finite and positive; the server refuses one coarser than 36,000 km. */
  readonly resolutionM: number;
}

/** A survey pass the server recorded over a whole body. */
export interface SurveyRecorded extends SurveyAsked {
  /** The level of the body's coarse field the pass covered, its every cell. */
  readonly level: number;
  /** The body's coverage revision with the pass folded in. */
  readonly revision: number;
}

/** What a whole body's pass is made of. */
export interface WholeBodySurvey {
  readonly universe: UniverseIdHex;
  readonly body: BodyIdHex;
  /** The level of the body's coarse field ({@link figureLevel}). */
  readonly level: number;
  readonly survey: SurveyAsked;
  /** The instant of the pass, its start and its end, within the universe's clock window. */
  readonly time: UniverseTime;
  /**
   * Where the server keeps it: the universe's survey log, `live`, for `--survey`; the scratch log
   * of training, `training`, for the body inspection, so that it never reaches the live game.
   */
  readonly scope: SurveyScopeDto;
}

/**
 * The `survey_pass` that records `survey` over every cell of the body's coarse field at `time`,
 * marked `training` on the wire for a training pass alone, so that a live one is R09's form.
 */
export function wholeBodySurveyPass(pass: WholeBodySurvey): RequestOf<"survey_pass"> {
  return {
    kind: "survey_pass",
    universe: pass.universe,
    body: pass.body,
    from: pass.time,
    to: pass.time,
    source: pass.survey.source,
    resolution_m: pass.survey.resolutionM,
    cells: [{ start: 0, end: coarseCellCount(pass.level) }],
    ...(pass.scope === "training" ? { scope: "training" } : {}),
  };
}
