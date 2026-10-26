/**
 * The views check's atmosphere run (plan R08, R08.T10.b; `--views-check --atmosphere`): a scripted
 * hand flight of `VIEW`'s photorealistic primary on the kept `ATMOSPHERE TEST`, by its controls, as
 * an operator would fly it: the moonlit night from the seat, then each air fixture at a few radii,
 * each captured for the looks by eye, with what the primary's label block said, for the main
 * process to judge and write.
 *
 * @remarks
 * In order:
 *
 * 1. `VIEW` is put on `ATMOSPHERE TEST`, the primary alone and photorealistic (the style key `4`),
 *    and the run waits, at most {@link AtmosphereCheckTiming.waitMs}, for the scene's media to land
 *    (`AtmosphereFixtures.media`) and for no `ATMOSPHERE: PENDING` to stand on the primary's label
 *    block. A failed load ends the run.
 * 2. The exposure is put under `AUTO` (`ENABLE`) on the `AVG` meter and, once `AUTO` has settled,
 *    the seat's look at the moon is captured as `moonlit-night`.
 * 3. The meter is put on `LIT`, the camera in `FREE` (the key `3`), and the tour flown
 *    ({@link ATMOSPHERE_TOUR}; the smoke's is its climb and its first fixture): for each leg the
 *    target keys (`]`) step to its body, which turns the free camera in place to look at its
 *    centre, and the leg is flown in stages ({@link atmosphereLegs}), the flight key `W` held on
 *    the primary's canvas for each at the slowest rate step (`PageUp`, `PageDown`) that flies it
 *    within {@link AtmosphereCheckTiming.legMaxS}. At the end of a fixture's leg,
 *    {@link ATMOSPHERE_GOAL_RADII} radii from it, once the camera's coast has run out, `AUTO` has
 *    settled and a few frames are drawn, it is captured as `fixture-<designation in lower kebab
 *    case>` ({@link fixtureCaptureLabel}). The tour first climbs from the seat towards `TEST MOON`,
 *    high in the night sky, since the line from the seat to the first fixture runs down through
 *    `TEST EARTH`.
 *
 * **`AUTO` settled.** Before each capture the run reads the `EXPOSURE` line's EV every
 * {@link AtmosphereCheckTiming.settleStepMs} until two readings in a row agree within
 * {@link AtmosphereCheckTiming.settleToleranceEv}: from `VIEW`'s opening exposure to a moonlit sky
 * is some 19 stops, which `AUTO` darkens at 1 EV/s. The wait is recorded with the capture
 * (`exposureSettleMs`); one past {@link AtmosphereCheckTiming.settleMaxMs} fails the run.
 *
 * **Staged legs, dead-reckoned.** Nothing on the page reads the free camera's range (the list's
 * ranges are the own ship's), so the run reckons it from the scene's geometry, the free camera's
 * rate and the primary's frames. Each hold of `W` is pressed and let go inside an animation frame
 * in which the primary has drawn (`AtmosphereCheckDeps.frame`), and the free camera, which steps
 * once in each of the primary's frames over the time since the one before (at most
 * `MAX_FREE_STEP_S`), and whose ramp and coast together fly its rate times its hold, so flies the
 * rate times the time between those two frames, its steps capped as it caps them
 * ({@link heldMs}): the run knows how far each hold flew, to a tick of the camera's integration.
 * What it cannot do is let go at the planned instant: the hold ends at the primary's frame
 * nearest it, half a frame either way at the hold's rate, which on a leg of 2 × 10¹¹ m, at
 * 10¹⁰ m/s, is some 10⁸ m, past the goal itself. So each leg is flown in stages along its line,
 * each ending at {@link ATMOSPHERE_STAGE_FRACTION} of its range from the target's centre or at
 * the goal, whichever is farther, each at its own rate step and flown from where the stage before
 * it was reckoned to end: a stage's error, a few percent of the range left after it, is taken up
 * by the next, and only the last, the slowest, leaves its own, a few percent of the goal at worst,
 * in the capture, whose range and phase are recorded as reckoned.
 *
 * Each capture records the primary's frame times since the capture before (the views probe's),
 * its label block's statements and its fault lines. The main process fails the run on any fault
 * line, or where no capture was taken.
 */

import type {
  AtmosphereCheckCapture,
  AtmosphereCheckFrames,
  AtmosphereCheckRecord,
  ViewsCheckApi,
} from "../../../../../preload/api";
import { add, norm, normalise, scale, sub, type Vec3 } from "../../../geometry/vec3";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { freeRateMPerS, MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
import {
  ATMOSPHERE_FIXTURES,
  ATMOSPHERE_MOON_CENTRE_M,
  ATMOSPHERE_MOON_RADIUS_M,
  ATMOSPHERE_SCENE_NAME,
  type AtmosphereFixtures,
  atmosphereSeatM,
  phaseAngleDeg,
} from "../../../view/scenes/atmosphereScene";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import { freeRateReading } from "../viewRun";
import { viewDriver } from "./viewsCheckRun";
import type { ViewsProbe, WindowFigures } from "./viewsProbe";

/** How far from each fixture's centre the camera stops, in its equatorial radii. */
export const ATMOSPHERE_GOAL_RADII = 8;

/**
 * Where each stage of a leg but its last ends: at this fraction of the range from the target's
 * centre it starts at, a twentieth, so that a stage's error is a few percent of the range left.
 */
export const ATMOSPHERE_STAGE_FRACTION = 1 / 20;

/** The statement a pending atmosphere stands as. */
const ATMOSPHERE_PENDING = ATMOSPHERE_STATEMENTS.atmospherePending;

/** `TEST MOON`'s designation, the climb's target. */
const MOON_DESIGNATION = "TEST MOON";

/**
 * A step of the tour: to a fixture, to {@link ATMOSPHERE_GOAL_RADII} radii from it, captured
 * there; or towards a body (a fixture or `TEST MOON`) by a distance, captured nowhere.
 */
export type AtmosphereTourStep =
  | { readonly kind: "fixture"; readonly designation: string }
  | { readonly kind: "toward"; readonly designation: string; readonly distanceM: number };

/**
 * The tour, from the seat on `TEST EARTH`'s night side: a climb of 5 × 10⁷ m, some eight of
 * `TEST EARTH`'s radii, towards `TEST MOON`, about 47° up, so that the camera leaves the ground
 * upwards, since the line from the seat to `TEST EARTH O3 SLAB` runs 30° below the horizon; then
 * each fixture on the scene's legs: `TEST EARTH O3 SLAB` a quarter of the way round the star,
 * seen at about 45° of phase, `TEST MARS` and `TEST SATURN` at about 60°, and back to
 * `TEST EARTH`, which so comes on its day side at about 45°.
 */
export const ATMOSPHERE_TOUR: ReadonlyArray<AtmosphereTourStep> = [
  { kind: "toward", designation: MOON_DESIGNATION, distanceM: 5e7 },
  { kind: "fixture", designation: "TEST EARTH O3 SLAB" },
  { kind: "fixture", designation: "TEST MARS" },
  { kind: "fixture", designation: "TEST SATURN" },
  { kind: "fixture", designation: "TEST EARTH" },
];

/** A smoke's tour: the climb and the first fixture. */
export const ATMOSPHERE_SMOKE_TOUR: ReadonlyArray<AtmosphereTourStep> = ATMOSPHERE_TOUR.slice(0, 2);

/** A capture a leg ends in: its label, its fixture, and its range and phase as planned. */
export interface AtmosphereLegCapture {
  readonly label: string;
  readonly designation: string;
  /** From the fixture's centre, m. */
  readonly rangeM: number;
  /** At the fixture, between the star and the camera, deg. */
  readonly phaseDeg: number;
}

/** One stage of a leg: a hold of `W` along the leg's line, to a range from its target's centre. */
export interface AtmosphereStage {
  /** Where it starts and ends as planned, m from the barycentre. */
  readonly fromM: Vec3;
  readonly toM: Vec3;
  /** Its length as planned, m. */
  readonly distanceM: number;
  /**
   * Its end's distance from the target's centre, m, at which the run ends it, flown from where
   * the stage before was reckoned to end.
   */
  readonly endRangeM: number;
}

/** One straight leg of the flight, along the line of sight to its target, flown in stages. */
export interface AtmosphereLeg {
  /** The body the camera is cut to, which it then looks at: a fixture, or `TEST MOON`. */
  readonly target: string;
  /** The target's centre, m from the barycentre. */
  readonly centreM: Vec3;
  /** Where it starts and ends as planned, m from the barycentre. */
  readonly fromM: Vec3;
  readonly toM: Vec3;
  /** Its length as planned, m. */
  readonly distanceM: number;
  /** Its stages, in order: the first starts at {@link fromM}, the last ends at {@link toM}. */
  readonly stages: ReadonlyArray<AtmosphereStage>;
  /** The capture at its end, or `null` for a leg towards a body (the climb). */
  readonly capture: AtmosphereLegCapture | null;
}

/**
 * A fixture's capture label: `fixture-` and its designation in lower kebab case,
 * `fixture-test-earth-o3-slab`, as `ViewsCheckApi.capture` takes one.
 *
 * @throws Error for a designation that makes no such label (other than letters, digits and
 *   spaces, or too long).
 */
export function fixtureCaptureLabel(designation: string): string {
  const label = `fixture-${designation.trim().toLowerCase().split(/\s+/u).join("-")}`;
  if (!/^[a-z0-9-]{1,48}$/u.test(label)) {
    throw new Error(`views check: ${designation} makes no capture label`);
  }
  return label;
}

/** A body the tour may fly to: its designation, centre and equatorial radius. */
interface TourBody {
  readonly designation: string;
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/** The air fixtures and `TEST MOON`, the bodies the tour may fly to. */
const TOUR_BODIES: ReadonlyArray<TourBody> = [
  ...ATMOSPHERE_FIXTURES.map((fixture) => ({
    designation: fixture.designation,
    centreM: fixture.centreM,
    radiusM: fixture.figure.equatorialRadiusM,
  })),
  {
    designation: MOON_DESIGNATION,
    centreM: ATMOSPHERE_MOON_CENTRE_M,
    radiusM: ATMOSPHERE_MOON_RADIUS_M,
  },
];

/**
 * The ranges from a target's centre at which a leg's stages end, from `startRangeM` to
 * `endRangeM`: each at `fraction` of the range it starts at, or at `endRangeM`, whichever is
 * farther, so that the last ends at `endRangeM`; none where the leg starts there or within it.
 *
 * @param fraction - Between 0 and 1, exclusive: {@link ATMOSPHERE_STAGE_FRACTION}.
 * @throws Error for a fraction outside (0, 1), or an end range not above zero.
 */
export function stageRangesM(
  startRangeM: number,
  endRangeM: number,
  fraction: number = ATMOSPHERE_STAGE_FRACTION,
): ReadonlyArray<number> {
  if (!(fraction > 0 && fraction < 1) || !(endRangeM > 0)) {
    throw new Error(`views check: no stages end at ${String(fraction)} of the range`);
  }
  const ends: number[] = [];
  let rangeM = startRangeM;
  while (rangeM > endRangeM) {
    rangeM = Math.max(rangeM * fraction, endRangeM);
    ends.push(rangeM);
  }
  return ends;
}

/**
 * The tour's legs from `startM` (the seat's eye), each along the line from where the one before
 * ended to its target's centre, in stages ({@link stageRangesM}): to {@link ATMOSPHERE_GOAL_RADII}
 * radii from a fixture, or `distanceM` towards a body.
 *
 * @throws Error for a tour naming a body the scene does not have, or a leg the camera could not
 *   fly forward (a start within the goal's distance, or a leg towards a body that would reach it).
 */
export function atmosphereLegs(
  startM: Vec3,
  tour: ReadonlyArray<AtmosphereTourStep>,
  goalRadii: number = ATMOSPHERE_GOAL_RADII,
  fraction: number = ATMOSPHERE_STAGE_FRACTION,
): ReadonlyArray<AtmosphereLeg> {
  const legs: AtmosphereLeg[] = [];
  let atM = startM;
  for (const step of tour) {
    const body = TOUR_BODIES.find((each) => each.designation === step.designation);
    if (body === undefined) {
      throw new Error(`views check: ATMOSPHERE TEST has no fixture ${step.designation}`);
    }
    const towardM = sub(body.centreM, atM);
    const startRangeM = norm(towardM);
    let endRangeM: number;
    switch (step.kind) {
      case "fixture":
        endRangeM = goalRadii * body.radiusM;
        break;
      case "toward":
        endRangeM = startRangeM - step.distanceM;
        if (!(endRangeM > body.radiusM)) {
          throw new Error(`views check: the leg towards ${step.designation} would reach it`);
        }
        break;
    }
    if (!(startRangeM > endRangeM)) {
      throw new Error(`views check: the leg to ${step.designation} starts within its goal`);
    }
    const direction = normalise(towardM);
    const fromM = atM;
    const stages: AtmosphereStage[] = [];
    let stageFromM = fromM;
    let stageFromRangeM = startRangeM;
    for (const stageEndRangeM of stageRangesM(startRangeM, endRangeM, fraction)) {
      const stageToM = add(fromM, scale(direction, startRangeM - stageEndRangeM));
      stages.push({
        fromM: stageFromM,
        toM: stageToM,
        distanceM: stageFromRangeM - stageEndRangeM,
        endRangeM: stageEndRangeM,
      });
      stageFromM = stageToM;
      stageFromRangeM = stageEndRangeM;
    }
    const toM = stageFromM;
    legs.push({
      target: body.designation,
      centreM: body.centreM,
      fromM,
      toM,
      distanceM: startRangeM - endRangeM,
      stages,
      capture:
        step.kind === "fixture"
          ? {
              label: fixtureCaptureLabel(body.designation),
              designation: body.designation,
              rangeM: endRangeM,
              phaseDeg: phaseAngleDeg(body.centreM, toM),
            }
          : null,
    });
    atM = toM;
  }
  return legs;
}

/**
 * The free camera's rate step that flies `distanceM` within `legMaxS`: the slowest, so that the
 * hold is as long as it may be and its ends' frames the least part of it; between 0 and `maxStep`.
 */
export function legRateStep(distanceM: number, legMaxS: number, maxStep: number): number {
  const step = Math.ceil(2 * Math.log10(distanceM / legMaxS));
  return Math.min(Math.max(step, 0), maxStep);
}

/** The highest rate step the run asks for: the kept system's own limit (`maxFreeRateStep`), 31. */
const MAX_RATE_STEP = 31;

/**
 * How long the free camera flew with its key held, ms, from the primary's draws from the frame
 * the key was pressed in to the frame it was let go in, both among them: the camera steps once a
 * draw, over the time since the one before, each step at most `MAX_FREE_STEP_S`.
 */
export function heldMs(drawTimesMs: ReadonlyArray<number>): number {
  const maxStepMs = 1000 * MAX_FREE_STEP_S;
  return drawTimesMs.reduce(
    (sumMs, atMs, i) =>
      sumMs + (i === 0 ? 0 : Math.min(atMs - (drawTimesMs[i - 1] ?? atMs), maxStepMs)),
    0,
  );
}

/** The `EXPOSURE` line's EV100 (`EV100 -3.0 AUTO`), or `null` where it reads none. */
export function exposureEv100(exposureLine: string | null): number | null {
  const match = /^EV100 (-?\d+(?:\.\d+)?)\b/u.exec(exposureLine ?? "");
  return match?.[1] === undefined ? null : Number(match[1]);
}

/**
 * The `q` quantile of `values` by nearest rank (the ⌈q n⌉-th smallest), or `null` for none: the
 * median at 0.5, the 95th percentile at 0.95.
 */
export function quantile(values: ReadonlyArray<number>, q: number): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = values.toSorted((a, b) => a - b);
  const rank = Math.min(sorted.length, Math.max(1, Math.ceil(q * sorted.length)));
  return sorted[rank - 1] ?? null;
}

/** A capture window's frame figures from the views probe's window over it, the primary's first. */
export function captureFrames(window: WindowFigures): AtmosphereCheckFrames {
  const primary = window.views[0];
  const gpuMs = primary?.gpuMs ?? [];
  return {
    fromMs: window.startMs,
    toMs: window.endMs,
    draws: primary?.draws ?? 0,
    intervalMedianMs: quantile(window.primaryIntervalsMs, 0.5),
    intervalP95Ms: quantile(window.primaryIntervalsMs, 0.95),
    gpuMedianMs: quantile(gpuMs, 0.5),
    gpuP95Ms: quantile(gpuMs, 0.95),
  };
}

/** What the primary's label block says: its lines' values, its statements and its fault lines. */
export interface PrimaryPlate {
  /** A line's value by its label (`CAMERA`, `EXPOSURE`), or `null` where it has none. */
  readonly line: (label: string) => string | null;
  readonly statements: ReadonlyArray<string>;
  readonly faults: ReadonlyArray<string>;
}

/** The primary's canvas, by its accessible name's slot, or `null`. */
function primaryCanvas(document: Document): HTMLCanvasElement | null {
  for (const canvas of document.querySelectorAll<HTMLCanvasElement>('canvas[role="application"]')) {
    if ((canvas.getAttribute("aria-label") ?? "").split(", ")[2] === PRIMARY_NAME) {
      return canvas;
    }
  }
  return null;
}

/**
 * The state of a request's status among the statements (`labels.ts`'s `atmosphereRejected`): set
 * as a status line, as the faults are, and in `--status-caution` for the server's own failure, but
 * a statement of the plate, never one of its faults.
 */
const REJECTED_STATE = "ATMOSPHERE REJECTED:";

/**
 * The primary's label block as it stands, found by its canvas's description (its `-label` part),
 * or `null` while it is not shown.
 *
 * @remarks
 * Its statements are its notes and its requests' statuses (`ATMOSPHERE REJECTED`), in order; its
 * fault lines every other status line, the graphics', the terrain's and the atmosphere's.
 */
export function primaryPlate(document: Document): PrimaryPlate | null {
  const ids = primaryCanvas(document)?.getAttribute("aria-describedby")?.split(" ") ?? [];
  const id = ids.find((each) => each.endsWith("-label"));
  const block = id === undefined ? null : document.getElementById(id);
  if (block === null) {
    return null;
  }
  const statements: string[] = [];
  const faults: string[] = [];
  for (const each of block.querySelectorAll(".view-label__statement, .request-status__text")) {
    const text = each.textContent.trim();
    if (each.classList.contains("view-label__statement") || text.startsWith(REJECTED_STATE)) {
      statements.push(text);
    } else {
      faults.push(text);
    }
  }
  return {
    line: (label) => {
      for (const line of block.querySelectorAll(".view-label__line")) {
        if (line.querySelector("dt")?.textContent.trim() === label) {
          return line.querySelector("dd output")?.textContent.trim() ?? null;
        }
      }
      return null;
    },
    statements,
    faults,
  };
}

/** The rate step whose `FREE` reading the `CAMERA` line holds (`FREE · RATE 1.00 km/s`), or `null`. */
export function rateStepOfCameraLine(cameraLine: string | null): number | null {
  for (let step = 0; step <= MAX_RATE_STEP; step += 1) {
    if (cameraLine === `FREE · ${freeRateReading(step)}`) {
      return step;
    }
  }
  return null;
}

/** How long each part of the atmosphere run takes. */
export interface AtmosphereCheckTiming {
  /** The longest wait for a control to take effect, and for the media, ms. */
  readonly waitMs: number;
  /** How often a wait looks again, ms. */
  readonly pollMs: number;
  /** How far apart the `EXPOSURE` line's EV is read while `AUTO` settles, ms. */
  readonly settleStepMs: number;
  /** How near two readings in a row must be for `AUTO` to count as settled, EV. */
  readonly settleToleranceEv: number;
  /** The longest wait for `AUTO` to settle before a capture, ms: past it the run fails. */
  readonly settleMaxMs: number;
  /** How long the free camera coasts after its key is let go before it is taken for at rest, ms. */
  readonly coastMs: number;
  /** The longest a stage's hold may be, s: its rate is the slowest that flies it within this. */
  readonly legMaxS: number;
  /** How long before a hold's planned end the run looks for its release frame by frame, ms. */
  readonly releaseLeadMs: number;
  /** The primary's draws waited for before each capture. */
  readonly framesBeforeCapture: number;
  /** The tour flown. */
  readonly tour: ReadonlyArray<AtmosphereTourStep>;
}

/**
 * A full run's timing (about five minutes) and a smoke's (the night, the climb and the first
 * fixture, about two, its stages' holds shorter).
 */
export const ATMOSPHERE_CHECK_TIMING: {
  readonly full: AtmosphereCheckTiming;
  readonly smoke: AtmosphereCheckTiming;
} = {
  full: {
    waitMs: 60_000,
    pollMs: 100,
    settleStepMs: 1_000,
    settleToleranceEv: 0.1,
    settleMaxMs: 60_000,
    coastMs: 3_000,
    legMaxS: 30,
    releaseLeadMs: 250,
    framesBeforeCapture: 5,
    tour: ATMOSPHERE_TOUR,
  },
  smoke: {
    waitMs: 60_000,
    pollMs: 100,
    settleStepMs: 1_000,
    settleToleranceEv: 0.1,
    settleMaxMs: 45_000,
    coastMs: 2_000,
    legMaxS: 10,
    releaseLeadMs: 250,
    framesBeforeCapture: 3,
    tour: ATMOSPHERE_SMOKE_TOUR,
  },
};

/** What the atmosphere run drives and reads. */
export interface AtmosphereCheckDeps {
  readonly api: Pick<ViewsCheckApi, "capture">;
  readonly views: Pick<ViewsProbe, "canvasOf" | "timer" | "faults" | "window" | "drawTimes">;
  /** The scene's media, which `SCENE_OPTIONS`' `ATMOSPHERE TEST` loads (`pageAtmosphereFixtures`). */
  readonly fixtures: Pick<AtmosphereFixtures, "media">;
  readonly document: Document;
  readonly timing: AtmosphereCheckTiming;
  /** The page's clock, `performance.now()`, on which the probe stamps frames. */
  readonly nowMs: () => number;
  readonly sleep: (ms: number) => Promise<void>;
  /**
   * Settles in the page's next animation frame with its timestamp, after `VIEW`'s own callbacks
   * in it: `requestAnimationFrame` through the probe's frame clock, asked from a task.
   */
  readonly frame: () => Promise<number>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly devicePixelRatio: number;
}

/** A key's press and release, dispatched on `target` as a keyboard would. */
function keyEvent(target: EventTarget, type: "keydown" | "keyup", key: string): void {
  target.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true }));
}

/** A fixture capture's place as the run reckoned it: its fixture, range and phase. */
interface ReckonedPlace {
  readonly designation: string;
  readonly rangeM: number;
  readonly phaseDeg: number;
}

/**
 * Runs the atmosphere run in `deps.document`'s `VIEW` and returns its record.
 *
 * @throws Error naming the step when a control does not take effect within the timing's wait, when
 *   the scene's media did not load, when `AUTO` did not settle within the timing's longest wait,
 *   when the camera was reckoned past a stage's end, or when the run is stopped through
 *   `deps.signal` (`STOPPED`).
 */
export async function runAtmosphereCheck(
  deps: AtmosphereCheckDeps,
): Promise<AtmosphereCheckRecord> {
  const { timing, views, document } = deps;
  const driver = viewDriver(deps);
  const { stopped, wait, until, controls } = driver;
  const captures: AtmosphereCheckCapture[] = [];
  let windowFromMs = deps.nowMs();

  const plate = (): PrimaryPlate => {
    const found = primaryPlate(document);
    if (found === null) {
      throw new Error("views check: the primary's label block is not shown");
    }
    return found;
  };
  const canvas = (): HTMLCanvasElement => {
    const found = primaryCanvas(document);
    if (found === null) {
      throw new Error("views check: the primary's canvas is not shown");
    }
    return found;
  };
  /** Presses one of `VIEW`'s single keys (`3`, `]`) with the focus off every canvas. */
  const pressSingle = (key: string): void => {
    stopped();
    keyEvent(document.body, "keydown", key);
    keyEvent(document.body, "keyup", key);
  };
  /** Waits until the primary has drawn `count` frames from now. */
  const framesDrawn = async (count: number): Promise<void> => {
    const fromMs = deps.nowMs();
    await until(
      `the primary did not draw ${String(count)} frames`,
      () => views.drawTimes(PRIMARY_VIEW_NAME, fromMs, Number.POSITIVE_INFINITY).length >= count,
    );
  };
  /**
   * Waits for the first animation frame in which the primary has drawn and `ready` holds of its
   * timestamp and the primary's draw before it (within a second, else `null`), acts in it, and
   * returns its timestamp.
   */
  const inPrimaryFrame = async (
    act: () => void,
    ready: (atMs: number, previousMs: number | null) => boolean,
  ): Promise<number> => {
    const deadline = deps.nowMs() + timing.waitMs;
    const look = async (): Promise<number> => {
      // From a task, so that VIEW's own callback for the frame comes before the run's.
      await wait(0);
      const atMs = await deps.frame();
      stopped();
      const drawn = views.drawTimes(PRIMARY_VIEW_NAME, atMs, Number.POSITIVE_INFINITY)[0] === atMs;
      const previousMs = views.drawTimes(PRIMARY_VIEW_NAME, atMs - 1000, atMs).at(-1) ?? null;
      if (drawn && ready(atMs, previousMs)) {
        act();
        return atMs;
      }
      if (deps.nowMs() > deadline) {
        throw new Error(
          `views check: the primary drew no frame to act in within ${String(timing.waitMs / 1000)} s`,
        );
      }
      return look();
    };
    return look();
  };
  /**
   * Waits until `AUTO` has settled before the capture `label`, the `EXPOSURE` line's EV read every
   * settle step until two readings in a row agree, and returns how long that took, ms.
   */
  const exposureSettled = async (label: string): Promise<number> => {
    const fromMs = deps.nowMs();
    const read = (): number => {
      const line = plate().line("EXPOSURE");
      const ev = exposureEv100(line);
      if (ev === null) {
        throw new Error(
          `views check: the EXPOSURE line read no EV before ${label} (${line ?? "none"})`,
        );
      }
      return ev;
    };
    const look = async (beforeEv: number): Promise<number> => {
      await wait(timing.settleStepMs);
      const ev = read();
      const elapsedMs = deps.nowMs() - fromMs;
      // The readings are to a tenth: the allowance keeps a tenth's step from failing on rounding.
      if (Math.abs(ev - beforeEv) <= timing.settleToleranceEv + 1e-9) {
        return elapsedMs;
      }
      if (elapsedMs >= timing.settleMaxMs) {
        throw new Error(
          `views check: AUTO did not settle before ${label} within ${String(timing.settleMaxMs / 1000)} s (the EXPOSURE line read EV100 ${beforeEv.toFixed(1)}, then ${ev.toFixed(1)})`,
        );
      }
      return look(ev);
    };
    return look(read());
  };
  /** Chooses the `EXPOSURE METER`'s `meter`. */
  const chooseMeter = async (meter: "AVG" | "LIT"): Promise<void> => {
    driver.controls.unfoldPanel("view-meter");
    const button = (): HTMLButtonElement | null => controls.meterButton(meter);
    await driver.press(button, `the EXPOSURE METER's ${meter}`);
    await until(
      `the ${meter} meter was not chosen`,
      () => button()?.getAttribute("aria-pressed") === "true",
    );
  };
  /** Puts the exposure under `AUTO` (`ENABLE`); `VIEW` opens under `MAN`. */
  const autoExposure = async (): Promise<void> => {
    const offered = (): boolean => {
      const button = controls.exposureButton("ENABLE");
      return button !== null && button.getAttribute("aria-disabled") !== "true";
    };
    controls.unfoldPanel("view-exposure");
    await until("the EXPOSURE panel's ENABLE was not offered", offered);
    stopped();
    controls.exposureButton("ENABLE")?.click();
    await until("AUTO did not resume on ENABLE", () => !offered());
  };
  /**
   * Captures the page as `label` once `AUTO` has settled, with what the primary's label block says
   * and its frames: a fixture's at its place as reckoned, or the night's (`null`).
   */
  const capture = async (label: string, place: ReckonedPlace | null): Promise<void> => {
    const exposureSettleMs = await exposureSettled(label);
    await framesDrawn(timing.framesBeforeCapture);
    const atMs = deps.nowMs();
    const said = plate();
    const window = views.window(windowFromMs, atMs, driver.shownViews());
    stopped();
    await deps.api.capture(label);
    const taken = {
      label,
      camera: said.line("CAMERA"),
      exposure: said.line("EXPOSURE"),
      exposureSettleMs,
      atMs,
      frames: captureFrames(window),
      statements: said.statements,
      faults: said.faults,
    };
    captures.push(
      place === null ? { kind: "night", ...taken } : { kind: "fixture", ...place, ...taken },
    );
    windowFromMs = deps.nowMs();
  };
  /** Steps the target keys until the primary's target is `designation`. */
  const selectTarget = async (designation: string): Promise<void> => {
    const selected = (): string | null =>
      document
        .querySelector(
          '[role="listbox"][aria-label="Marks in view"] [role="option"][aria-selected="true"]',
        )
        ?.getAttribute("aria-label") ?? null;
    const isTarget = (): boolean => selected()?.startsWith(`${designation},`) === true;
    const options = document.querySelectorAll(
      '[role="listbox"][aria-label="Marks in view"] [role="option"]',
    ).length;
    const step = async (left: number): Promise<void> => {
      if (isTarget()) {
        return;
      }
      if (left === 0) {
        throw new Error(`views check: the target keys did not reach ${designation}`);
      }
      const before = selected();
      pressSingle("]");
      await until(
        `the target keys did not step from ${before ?? "none"}`,
        () => selected() !== before,
      );
      await step(left - 1);
    };
    await step(Math.max(options, 1) + 1);
  };
  /** Sets the free camera's rate to `wanted` through `PageUp` and `PageDown` on the primary's canvas. */
  const setRate = async (wanted: number): Promise<void> => {
    const current = (): number | null => rateStepOfCameraLine(plate().line("CAMERA"));
    const step = async (): Promise<void> => {
      const now = current();
      if (now === null) {
        throw new Error("views check: the CAMERA line reads no FREE rate");
      }
      if (now === wanted) {
        return;
      }
      stopped();
      const key = now < wanted ? "PageUp" : "PageDown";
      keyEvent(canvas(), "keydown", key);
      keyEvent(canvas(), "keyup", key);
      await until(`the rate did not step from ${String(now)}`, () => current() !== now);
      await step();
    };
    await step();
  };
  /**
   * Flies `distanceM` straight ahead: the rate set, `W` held from one of the primary's frames to
   * the one nearest the hold's planned end, and the coast; returns how far it flew as reckoned, m.
   */
  const fly = async (distanceM: number): Promise<number> => {
    const rateStep = legRateStep(distanceM, timing.legMaxS, MAX_RATE_STEP);
    await setRate(rateStep);
    const rateMPerS = freeRateMPerS(rateStep);
    const holdMs = (1000 * distanceM) / rateMPerS;
    const target = canvas();
    let releasedAtMs: number | null = null;
    const pressedAtMs = await inPrimaryFrame(
      () => {
        keyEvent(target, "keydown", "w");
      },
      () => true,
    );
    try {
      await wait(Math.max(0, holdMs - timing.releaseLeadMs));
      const endMs = pressedAtMs + holdMs;
      releasedAtMs = await inPrimaryFrame(
        () => {
          keyEvent(target, "keyup", "w");
        },
        // The frame nearest the planned end: the next is taken to come an interval later.
        (atMs, previousMs) => atMs + (atMs - (previousMs ?? atMs)) / 2 >= endMs,
      );
    } finally {
      if (releasedAtMs === null) {
        keyEvent(target, "keyup", "w");
      }
    }
    const held = heldMs(views.drawTimes(PRIMARY_VIEW_NAME, pressedAtMs, releasedAtMs + 1e-3));
    await wait(timing.coastMs);
    return (rateMPerS * held) / 1000;
  };

  await until("VIEW did not show its primary view", () => controls.styleOf(PRIMARY_NAME) !== null);
  await driver.chooseScene(ATMOSPHERE_SCENE_NAME);
  const chosenAtMs = deps.nowMs();
  await driver.configure({ primary: "photorealistic", instruments: [null, null] });
  await until("ATMOSPHERE TEST's media did not load", () => deps.fixtures.media.kind !== "loading");
  if (deps.fixtures.media.kind === "failed") {
    throw new Error("views check: ATMOSPHERE TEST's media did not load");
  }
  await until(
    `${ATMOSPHERE_PENDING} still stood`,
    () => !plate().statements.includes(ATMOSPHERE_PENDING),
  );
  const mediaReadyMs = deps.nowMs() - chosenAtMs;

  await autoExposure();
  await chooseMeter("AVG");
  await capture("moonlit-night", null);

  await chooseMeter("LIT");
  pressSingle("3");
  await until("FREE was not taken", () => plate().line("CAMERA")?.startsWith("FREE") === true);
  const legs = atmosphereLegs(atmosphereSeatM(), timing.tour);
  let reckonedM = atmosphereSeatM();
  /** Flies `leg`'s stages from `index` on, each from where the camera is reckoned to be. */
  const stages = async (leg: AtmosphereLeg, index: number): Promise<void> => {
    const stage = leg.stages[index];
    if (stage === undefined) {
      return;
    }
    const towardM = sub(leg.centreM, reckonedM);
    const distanceM = norm(towardM) - stage.endRangeM;
    if (!(distanceM > 0)) {
      throw new Error(
        `views check: the camera was reckoned ${(-distanceM / 1000).toFixed(0)} km past the end of stage ${String(index + 1)} towards ${leg.target}`,
      );
    }
    const flownM = await fly(distanceM);
    reckonedM = add(reckonedM, scale(normalise(towardM), flownM));
    await stages(leg, index + 1);
  };
  const tour = async (index: number): Promise<void> => {
    const leg = legs[index];
    if (leg === undefined) {
      return;
    }
    await selectTarget(leg.target);
    await stages(leg, 0);
    if (leg.capture !== null) {
      await capture(leg.capture.label, {
        designation: leg.capture.designation,
        rangeM: norm(sub(leg.centreM, reckonedM)),
        phaseDeg: phaseAngleDeg(leg.centreM, reckonedM),
      });
    }
    await tour(index + 1);
  };
  await tour(0);

  return {
    timer: views.timer(),
    devicePixelRatio: deps.devicePixelRatio,
    canvas: views.canvasOf(PRIMARY_VIEW_NAME),
    mediaReadyMs,
    captures,
    faults: views.faults(),
  };
}

/** What {@link recordAtmosphereCheck} asks of the main process. */
export type AtmosphereCheckEnding = Pick<ViewsCheckApi, "writeAtmosphereResults" | "end">;

/**
 * Runs `run` and ends the check with its outcome: the record written (the main process judges
 * it) and the run ended, or the run failed with the reason it threw; nothing at all once `signal`
 * has stopped it.
 */
export async function recordAtmosphereCheck(
  api: AtmosphereCheckEnding,
  run: () => Promise<AtmosphereCheckRecord>,
  signal: AbortSignal,
): Promise<void> {
  try {
    const record = await run();
    if (signal.aborted) {
      return;
    }
    await api.writeAtmosphereResults(record);
    await api.end({ status: "pass" });
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    const reason = error instanceof Error ? error.message : String(error);
    console.error("views check: the atmosphere run failed:", error);
    await api.end({ status: "fail", reason });
  }
}
