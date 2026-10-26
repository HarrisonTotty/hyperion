/**
 * The views check's atmosphere run (plan R08, R08.T10.b; `--views-check --atmosphere`): a scripted
 * hand flight of `VIEW`'s photorealistic primary on the kept `ATMOSPHERE TEST`, by its controls, as
 * an operator would fly it, past each air fixture at a few radii and then to the moonlit night, each
 * captured for the looks by eye, with what the primary's label block said, for the main process to
 * judge and write.
 *
 * @remarks
 * In order:
 *
 * 1. `VIEW` is put on `ATMOSPHERE TEST`, the primary alone and photorealistic (the style key `4`),
 *    and the run waits, at most {@link AtmosphereCheckTiming.waitMs}, for the scene's media to land
 *    (`AtmosphereFixtures.media`) and for no `ATMOSPHERE: PENDING` to stand on the primary's label
 *    block. A failed load ends the run.
 * 2. The exposure is put under `AUTO` (`ENABLE`) on the `AVG` meter and, after
 *    {@link AtmosphereCheckTiming.settleMs}, the seat's look at the moon is captured as
 *    `moonlit-night`.
 * 3. The meter is put on `LIT`, the camera in `FREE` (the key `3`), and the tour flown
 *    ({@link ATMOSPHERE_TOUR}, the smoke's its first fixture alone): for each fixture the target
 *    keys (`]`) step to it, which turns the free camera to look at its centre, and the flight key
 *    `W` is held on the primary's canvas at the slowest rate step (`PageUp`, `PageDown`) that flies
 *    the leg within {@link AtmosphereCheckTiming.legMaxS}, until the camera stands
 *    {@link ATMOSPHERE_GOAL_RADII} radii from it ({@link atmosphereLegs}); once its coast has run
 *    out, the exposure settled and a few frames drawn, it is captured as
 *    `fixture-<designation in lower kebab case>` ({@link fixtureCaptureLabel}). Before `TEST EARTH`
 *    the camera flies towards the star first, so that it comes on the planet's day side.
 *
 * The flight is dead-reckoned from the scene's geometry and the free camera's rate: a hold's
 * length is good to a frame or two at each end, a percent of a leg at worst, so each capture's
 * range is recorded as flown, not measured, and the legs are long enough, and the goal far enough
 * out, that no error of that size takes the camera into a fixture.
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
import { freeRateMPerS } from "../../../view/camera/freeCamera";
import {
  ATMOSPHERE_FIXTURES,
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

/** The statement a pending atmosphere stands as. */
const ATMOSPHERE_PENDING = ATMOSPHERE_STATEMENTS.atmospherePending;

/** The star's designation, which the target keys step to for a leg towards it. */
export const ATMOSPHERE_STAR_DESIGNATION = "TEST STAR";

/** A step of the tour: to a fixture, captured there, or towards the star by a distance. */
export type AtmosphereTourStep =
  | { readonly kind: "fixture"; readonly designation: string }
  | { readonly kind: "sunward"; readonly distanceM: number };

/**
 * The tour, from the seat on `TEST EARTH`'s night side: each fixture after it on the scene's legs,
 * seen at about 60° of phase; then 2 × 10⁹ m towards the star and back to `TEST EARTH`, which so
 * comes at about 60° of phase rather than 120°.
 */
export const ATMOSPHERE_TOUR: ReadonlyArray<AtmosphereTourStep> = [
  { kind: "fixture", designation: "TEST EARTH SLAB" },
  { kind: "fixture", designation: "TEST MARS" },
  { kind: "fixture", designation: "TEST SATURN" },
  { kind: "sunward", distanceM: 2e9 },
  { kind: "fixture", designation: "TEST EARTH" },
];

/** A capture a leg ends in: its label, its fixture, and its range and phase as flown. */
export interface AtmosphereLegCapture {
  readonly label: string;
  readonly designation: string;
  /** From the fixture's centre, m. */
  readonly rangeM: number;
  /** At the fixture, between the star and the camera, deg. */
  readonly phaseDeg: number;
}

/** One straight leg of the flight, along the line of sight to its target. */
export interface AtmosphereLeg {
  /** The target the camera is cut to, which it then looks at: a fixture or the star. */
  readonly target: string;
  /** Where it starts and ends, m from the barycentre, as dead-reckoned. */
  readonly fromM: Vec3;
  readonly toM: Vec3;
  /** Its length, m. */
  readonly distanceM: number;
  /** The capture at its end, or `null` for a leg towards the star. */
  readonly capture: AtmosphereLegCapture | null;
}

/**
 * A fixture's capture label: `fixture-` and its designation in lower kebab case,
 * `fixture-test-earth-slab`, as `ViewsCheckApi.capture` takes one.
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

/**
 * The tour's legs from `startM` (the seat's eye), each along the line from where the one before
 * ended to its target's centre: to {@link ATMOSPHERE_GOAL_RADII} radii from a fixture, or
 * `distanceM` towards the star at the barycentre.
 *
 * @throws Error for a tour naming a fixture the scene does not have, or a leg the camera could not
 *   fly forward (a start within the goal's distance).
 */
export function atmosphereLegs(
  startM: Vec3,
  tour: ReadonlyArray<AtmosphereTourStep>,
  goalRadii: number = ATMOSPHERE_GOAL_RADII,
): ReadonlyArray<AtmosphereLeg> {
  const legs: AtmosphereLeg[] = [];
  let atM = startM;
  for (const step of tour) {
    let leg: AtmosphereLeg;
    switch (step.kind) {
      case "fixture": {
        const fixture = ATMOSPHERE_FIXTURES.find((each) => each.designation === step.designation);
        if (fixture === undefined) {
          throw new Error(`views check: ATMOSPHERE TEST has no fixture ${step.designation}`);
        }
        const towardM = sub(fixture.centreM, atM);
        const rangeM = goalRadii * fixture.figure.equatorialRadiusM;
        const distanceM = norm(towardM) - rangeM;
        if (!(distanceM > 0)) {
          throw new Error(`views check: the leg to ${step.designation} starts within its goal`);
        }
        const toM = add(atM, scale(normalise(towardM), distanceM));
        leg = {
          target: fixture.designation,
          fromM: atM,
          toM,
          distanceM,
          capture: {
            label: fixtureCaptureLabel(fixture.designation),
            designation: fixture.designation,
            rangeM,
            phaseDeg: phaseAngleDeg(fixture.centreM, toM),
          },
        };
        break;
      }
      case "sunward": {
        const toM = add(atM, scale(normalise(scale(atM, -1)), step.distanceM));
        leg = {
          target: ATMOSPHERE_STAR_DESIGNATION,
          fromM: atM,
          toM,
          distanceM: step.distanceM,
          capture: null,
        };
        break;
      }
    }
    legs.push(leg);
    atM = leg.toM;
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
  /** From an arrival, or the night's exposure set, to its capture: `AUTO`'s settling, ms. */
  readonly settleMs: number;
  /** How long the free camera coasts after its key is let go before it is taken for at rest, ms. */
  readonly coastMs: number;
  /** The longest a leg's hold may be, s: the rate is the slowest that flies it within this. */
  readonly legMaxS: number;
  /** The primary's draws waited for before each capture. */
  readonly framesBeforeCapture: number;
  /** The tour flown. */
  readonly tour: ReadonlyArray<AtmosphereTourStep>;
}

/** A full run's timing (about four minutes) and a smoke's (its first fixture alone, about one). */
export const ATMOSPHERE_CHECK_TIMING: {
  readonly full: AtmosphereCheckTiming;
  readonly smoke: AtmosphereCheckTiming;
} = {
  full: {
    waitMs: 60_000,
    pollMs: 100,
    settleMs: 4_000,
    coastMs: 3_000,
    legMaxS: 30,
    framesBeforeCapture: 5,
    tour: ATMOSPHERE_TOUR,
  },
  smoke: {
    waitMs: 60_000,
    pollMs: 100,
    settleMs: 1_500,
    coastMs: 3_000,
    legMaxS: 30,
    framesBeforeCapture: 3,
    tour: ATMOSPHERE_TOUR.slice(0, 1),
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
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly devicePixelRatio: number;
}

/** A key's press and release, dispatched on `target` as a keyboard would. */
function keyEvent(target: EventTarget, type: "keydown" | "keyup", key: string): void {
  target.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true }));
}

/**
 * Runs the atmosphere run in `deps.document`'s `VIEW` and returns its record.
 *
 * @throws Error naming the step when a control does not take effect within the timing's wait, when
 *   the scene's media did not load, or when the run is stopped through `deps.signal` (`STOPPED`).
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
  /** Captures the page as `label`, with what the primary's label block says and its frames. */
  const capture = async (label: string, flown: AtmosphereLegCapture | null): Promise<void> => {
    await framesDrawn(timing.framesBeforeCapture);
    const atMs = deps.nowMs();
    const said = plate();
    const window = views.window(windowFromMs, atMs, driver.shownViews());
    stopped();
    await deps.api.capture(label);
    captures.push({
      label,
      designation: flown?.designation ?? null,
      camera: said.line("CAMERA"),
      rangeM: flown?.rangeM ?? null,
      phaseDeg: flown?.phaseDeg ?? null,
      exposure: said.line("EXPOSURE"),
      atMs,
      frames: captureFrames(window),
      statements: said.statements,
      faults: said.faults,
    });
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
  /** Flies `leg`: its target cut to, the rate set, `W` held for the leg's time, and the coast. */
  const fly = async (leg: AtmosphereLeg): Promise<void> => {
    await selectTarget(leg.target);
    const rateStep = legRateStep(leg.distanceM, timing.legMaxS, MAX_RATE_STEP);
    await setRate(rateStep);
    const holdMs = (1000 * leg.distanceM) / freeRateMPerS(rateStep);
    const target = canvas();
    stopped();
    keyEvent(target, "keydown", "w");
    try {
      await wait(holdMs);
    } finally {
      keyEvent(target, "keyup", "w");
    }
    await wait(timing.coastMs);
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
  await wait(timing.settleMs);
  await capture("moonlit-night", null);

  await chooseMeter("LIT");
  pressSingle("3");
  await until("FREE was not taken", () => plate().line("CAMERA")?.startsWith("FREE") === true);
  const legs = atmosphereLegs(atmosphereSeatM(), timing.tour);
  const tour = async (index: number): Promise<void> => {
    const leg = legs[index];
    if (leg === undefined) {
      return;
    }
    await fly(leg);
    if (leg.capture !== null) {
      await wait(timing.settleMs);
      await capture(leg.capture.label, leg.capture);
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
