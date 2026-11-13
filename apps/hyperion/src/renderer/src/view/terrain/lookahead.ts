/**
 * The lookahead demand: bakes asked ahead of a view's motion (plan R10, R10.T14.b; the
 * seamless-lookahead lane, 2026-11-01).
 *
 * @remarks
 * A selection's demand is its pose's alone, opened only by `motion`'s frustum margin (at most
 * 0.1 rad), and the streaming gate lets selection reach one level below what is baked, so a
 * moving view waits about a bake round trip for each level it needs. At each selection the terrain
 * pass therefore selects a second time, by the same pure `selectPatches`, for each view that moves
 * ({@link moving}), at its pose extrapolated by v × L ({@link aheadPositionM}): v its velocity over the last two steps, L the
 * pass's own demand → resident p50 over its last {@link LOOKAHEAD_SAMPLES} bakes
 * ({@link lookaheadSeconds}). That selection is never drawn: its demand joins the pass's at
 * {@link LOOKAHEAD_DEMAND_WEIGHT} of each view's weight (`priority.ts`' `joinDemand`), its patches
 * pinned only as demanded, so that the cache gives them up before the selection's own. A view
 * closing on the ground selects ahead at the lower altitude, so that the next level's patches
 * beneath it are asked a round trip early. Ties of priority go to the request the views reach first
 * ({@link timeToNeedS}).
 */

import { add, dot, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { vertexDir } from "./cube";
import type { PatchKey } from "./patchKey";
import { type PlanetGeometry, surfacePoint } from "./planet";
import { LOOKAHEAD_DEMAND_WEIGHT } from "./priority";
import type { SelectionInput, ViewSelectionInput } from "./select";

/** The least lookahead L, s: fifteen frames at 60 Hz, a hand value. */
export const LOOKAHEAD_MIN_S = 0.25;

/**
 * The most lookahead L, s: a hand value, so that a backlog does not send the second selection far
 * from where the view will be, the straight line's error growing with L².
 */
export const LOOKAHEAD_MAX_S = 2;

/** How many of the pass's latest bakes L is the median of. */
export const LOOKAHEAD_SAMPLES = 64;

/** How many of the pass's latest steps the selection cost is taken over, for the lookahead's cadence. */
export const LOOKAHEAD_COST_STEPS = 64;

/**
 * The p95 of a step's selection time, ms, over {@link LOOKAHEAD_COST_STEPS} steps, above which the
 * lookahead selects at most every {@link LOOKAHEAD_SLOW_EVERY}th selection: an eighth of a 60 Hz
 * frame.
 */
export const LOOKAHEAD_COST_P95_MS = 2;

/** The lookahead's cadence, in selections, while selection is slow ({@link LOOKAHEAD_COST_P95_MS}). */
export const LOOKAHEAD_SLOW_EVERY = 4;

/**
 * The most of a view's nearest ground distance d_min its lookahead pose closes on the body by: half,
 * so that a view descending fast never selects from beneath the ground it closes on.
 */
export const LOOKAHEAD_MAX_CLOSING_FRACTION = 0.5;

/**
 * Whether selection is slow: the p95 of `stepCostsMs`, the latest steps' selection times (0 for a
 * step that did not select), is above {@link LOOKAHEAD_COST_P95_MS}; never with none.
 *
 * @remarks
 * The p95 is by nearest rank: the ⌈0.95 n⌉th smallest of n.
 */
export function slowSelection(stepCostsMs: ReadonlyArray<number>): boolean {
  if (stepCostsMs.length === 0) {
    return false;
  }
  const sorted = stepCostsMs.toSorted((a, b) => a - b);
  const p95 = sorted[Math.ceil(0.95 * sorted.length) - 1] ?? 0;
  return p95 > LOOKAHEAD_COST_P95_MS;
}

/** A view's motion, as the lookahead takes it. */
export interface ViewMotion {
  /** Its velocity, body-fixed metres a second, over the last two steps. */
  readonly velocityMps: Vec3;
  /**
   * Its nearest ground's distance d_min, metres: the move rule's, the box distance to its nearest
   * selected patch not at the finest level, or its nearest surveyed ground; `+∞` where none is known.
   */
  readonly nearestM: number;
}

/**
 * L, s: the median of `roundTripsS`, the pass's latest demand → resident times, within
 * [{@link LOOKAHEAD_MIN_S}, {@link LOOKAHEAD_MAX_S}]; {@link LOOKAHEAD_MIN_S} with none.
 *
 * @remarks
 * The median of an even count is the mean of the middle two.
 */
export function lookaheadSeconds(roundTripsS: ReadonlyArray<number>): number {
  if (roundTripsS.length === 0) {
    return LOOKAHEAD_MIN_S;
  }
  const sorted = roundTripsS.toSorted((a, b) => a - b);
  const mid = sorted.length >> 1;
  const median =
    sorted.length % 2 === 1
      ? (sorted[mid] ?? 0)
      : ((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2;
  return Math.min(LOOKAHEAD_MAX_S, Math.max(LOOKAHEAD_MIN_S, median));
}

/**
 * Whether a view selects ahead: it moved since its last step, with its nearest ground known.
 *
 * @remarks
 * At any speed (lean, 2026-11-01), not only faster than `RESELECT_MOVE_FRACTION` × d_min a
 * second: a view closing on the ground even at a few per cent of its height a second has a split
 * cross its tolerance at the pose ahead first, and its children asked a round trip early. TERRAIN
 * TEST's descent closes at 8.25% of its height a second at most, below that rule's 9.09%, and
 * looking ahead at its every motion cuts its frames with a stand-in from 36% to 24% (40–180 s, high,
 * 3 × 41 ms). A still view selects nothing more, and the cadence bounds the cost.
 */
export function moving(motion: ViewMotion): boolean {
  const { velocityMps: v, nearestM } = motion;
  return Number.isFinite(nearestM) && (v.x !== 0 || v.y !== 0 || v.z !== 0);
}

/**
 * Where a view at `positionM` will be `aheadS` seconds on at its velocity, body-fixed metres:
 * p + v × L, but closing on the body's centre by at most {@link LOOKAHEAD_MAX_CLOSING_FRACTION} of
 * its nearest ground's distance; along the ground the motion is as given.
 */
export function aheadPositionM(positionM: Vec3, motion: ViewMotion, aheadS: number): Vec3 {
  const step = scale(motion.velocityMps, aheadS);
  const r = Math.hypot(positionM.x, positionM.y, positionM.z);
  if (!(r > 0)) {
    return add(positionM, step);
  }
  const up = scale(positionM, 1 / r);
  // Negative where the step closes on the body.
  const radialM = dot(step, up);
  const closingM = LOOKAHEAD_MAX_CLOSING_FRACTION * motion.nearestM;
  if (radialM >= -closingM) {
    return add(positionM, step);
  }
  return add(positionM, add(step, scale(up, -closingM - radialM)));
}

/**
 * The lookahead's selection input: `input` with each view that is {@link moving} at its
 * {@link aheadPositionM}, its weight × {@link LOOKAHEAD_DEMAND_WEIGHT}, and the views that are not
 * left out; `null` where none moves, when nothing more is asked.
 *
 * @param motions - Each view's motion, in `input.views`' order, `null` for a view with none yet.
 * @param aheadS - L, s ({@link lookaheadSeconds}).
 */
export function lookaheadInput(
  input: SelectionInput,
  motions: ReadonlyArray<ViewMotion | null>,
  aheadS: number,
): SelectionInput | null {
  const views: ViewSelectionInput[] = [];
  for (let n = 0; n < input.views.length; n += 1) {
    const view = input.views[n];
    const motion = motions[n] ?? null;
    if (view === undefined || motion === null || !moving(motion)) {
      continue;
    }
    views.push({
      ...view,
      camera: {
        positionM: aheadPositionM(view.camera.positionM, motion, aheadS),
        orientation: view.camera.orientation,
      },
      weight: view.weight * LOOKAHEAD_DEMAND_WEIGHT,
    });
  }
  return views.length === 0 ? null : { ...input, views };
}

/** A moving view's camera, as {@link timeToNeedS} reads it. */
export interface MovingCamera {
  /** Its position, body-fixed metres. */
  readonly positionM: Vec3;
  /** Its velocity, body-fixed metres a second. */
  readonly velocityMps: Vec3;
}

/**
 * When the moving views reach patch `key`, s: the least over `cameras` of the time at which the
 * camera, along its straight line, passes nearest the patch's centre on the datum, within
 * [0, `aheadS`]: 0 for a patch at or behind it, `aheadS` for one beyond its lookahead.
 *
 * @remarks
 * It breaks ties of priority alone (`priority.ts`' `compareRequests`): among requests of equal
 * weighted excess, siblings under one stand-in among them, the one the views reach first is baked
 * first. With no camera it is 0.
 */
export function timeToNeedS(
  planet: PlanetGeometry,
  key: PatchKey,
  cameras: ReadonlyArray<MovingCamera>,
  aheadS: number,
): number {
  if (cameras.length === 0) {
    return 0;
  }
  const [x, y, z] = surfacePoint(planet.figure, vertexDir(key, 32, 32), 0);
  const centre = vec3(x, y, z);
  let least = aheadS;
  for (const camera of cameras) {
    const v = camera.velocityMps;
    const speed2 = dot(v, v);
    const t = speed2 > 0 ? dot(sub(centre, camera.positionM), v) / speed2 : 0;
    least = Math.min(least, Math.max(0, t));
  }
  return least;
}
