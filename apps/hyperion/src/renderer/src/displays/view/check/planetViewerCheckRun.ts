/**
 * The planet viewer run (plan R15, T6): the `PLANET VIEWER`'s planet captured from orbit, from low
 * orbit and on its surface, driven through the consoles as an operator would.
 *
 * @remarks
 * 1. The first universe the server lists is opened on `GALAXY`, and the `PLANET VIEWER` shown.
 * 2. `GENERATE` shows the panel's inputs as they stand at mount, an Earth about a Sun, in `VIEW`'s
 *    `ORBIT` camera. Once it is drawn photorealistic, lit and its atmosphere received, the run waits
 *    for the server's warm-up, its sky's first reply ({@link skyAnswered}), within the draw's wait,
 *    and goes on past it, so that the ground's wait starts once the server has built the galaxy's
 *    tables and its census's first step, which on a cold server hold every worker (R15.T6's
 *    finding: a first launch on a cold server could outwait the ground's wait). Once its
 *    ground is surveyed and `AUTO` settled, the page is captured as `planet-viewer-orbit`. Then the
 *    `INPUTS` column is captured with each part that folds open in turn, `SHOWN`, then `ORBIT` and
 *    `PLANET` where the compact layout folds them, then the volatile ranks, and at its tallest, the
 *    ranks open, with `SHOWN` where the layout lets it stand beside them, `SUN` `HELD` and a rank's
 *    entry refused ({@link PLANET_VIEWER_INPUTS_CAPTURES}), and folded again (R15.T6's fold).
 * 3. The wheel zooms the camera to {@link LOW_ORBIT_HEIGHT_M} above the ground at its line of sight,
 *    which it looks down along at the centre, read on the lower plate as `SLANT RANGE FROM CAMERA`
 *    beside the ground's `ELEVATION`, from which the steps above the datum are worked out (the
 *    camera zooms its altitude above the datum, `orbitZoomed`, and is held above it, so over ground
 *    below the datum it goes to the height above the datum); once the terrain has streamed and
 *    `AUTO` settled, the page is captured as `planet-viewer-low-orbit`.
 *    The arrows then move it on over the ground until the ground under it stands
 *    {@link HIGH_GROUND_M} above the datum, at most {@link HIGH_GROUND_TRIES} moves, so that the
 *    floor's look is over high ground.
 * 4. Zoomed again to {@link SURFACE_HEIGHT_M} above the ground, the look is tilted to the horizon
 *    under `R`, held on the view's canvas, and the page is captured as `planet-viewer-surface`.
 * 5. The look is tilted back down under `F` and the wheel takes the camera down to its floor, 10 m
 *    above the drawn ground under it (plan R15, T4), where the ground at the line of sight must read
 *    at least half that below it, so that the camera stands over the ground's face, never inside
 *    it; the page is captured as `planet-viewer-floor`.
 * 6. Last, `SMA` is moved inside the star and `GENERATE` pressed, which the server refuses, so that
 *    `SMA`'s reason stands under its field and `GENERATE`'s outcome names it, and with `SUN` `HELD`
 *    and `LATITUDE`'s entry refused the column is captured as `planet-viewer-inputs-refused`.
 * 7. Then a moon of the galaxy (R15.T5's follow-up): the first of the real planets `viewer_randomize`
 *    draws, from draw seed 1, at most {@link MOON_SEARCH_DRAWS}, whose system holds a moon present
 *    at the epoch gives it ({@link firstMoon}). With `SUN` `ORBITAL` again its ID is entered in
 *    `BODY`, and once the panel holds `GENERATE` back with `NO INPUTS` and the moon is drawn
 *    photorealistic, no longer `PHOTOREALISTIC: PREPARING`, it is captured from orbit as
 *    `planet-viewer-moon`.
 *
 * Each capture is taken twice: of the whole tab, and with `MAXIMIZE VIEW` on (`-maximized`), its
 * label block and panels hidden, so that the planet fills the view.
 *
 * Given a count of draws (`--randomize`; the owner's report, 2026-11-05), the run instead presses
 * `RANDOMIZE` that many times, draw seeds 1 on, and captures each draw's planet from orbit under
 * each `WHITE BALANCE`, `HOST` then `D65` ({@link randomizeCaptureLabel}), once its own scene is
 * drawn, lit, its air and its terrain no longer pending and `AUTO` settled, logging each draw's
 * inputs and white balance line; a draw's lit body is logged rather than required. Where the lower plate holds its
 * readout back for the room it is read under `MAXIMIZE VIEW` too. The run fails,
 * naming its step, where the server refuses what it asks, where a control does not take effect or
 * a reading does not come within its wait, where the tab overflows the window or the `INPUTS`
 * column's foot is cut ({@link inputsCutPx}), where the primary's
 * label block shows a fault line or
 * `LIGHTING: NOT RECEIVED`, where a capture's frames drew no lit body larger than a point, where the
 * planet's ground is not modelled, or where it is stopped (`STOPPED`).
 */
import {
  type BodyIdHex,
  parseBodyId,
  type RequestClient,
  type ResponseFor,
  type UniverseIdHex,
  type UniverseInfo,
} from "@hyperion/protocol";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { ORBIT_FLOOR_M, ORBIT_ZOOM_FACTOR } from "../../../view/camera/orbit";
import { SKY_PENDING } from "../../../view/sky/label";
import { toBodiesRequest } from "../../../lib/system/bodiesWire";
import { GENERATE_KEY, NO_INPUTS_REASON } from "../../planet/PlanetInputsPanel";
import { RANDOMIZE_KEY } from "../../planet/PlanetViewerBar";
import { type WhiteBalance, whiteBalanceName } from "../../../view/post/whiteBalance";
import { MISSING_READING as MISSING, WHITE_BALANCE_LABEL } from "../viewRun";
import { MAXIMIZE_KEY } from "../displayKeys";
import { PHOTOREAL_PREPARING } from "../viewRun";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import {
  exposureEv100,
  primaryCanvas,
  primaryPlate,
  type PrimaryPlate,
} from "./atmosphereCheckRun";
import {
  inspectionGround,
  inspectionLighting,
  LitBodyDraws,
  lowerPlateLines,
  readoutHeldBack,
  slantRangeM,
} from "./inspectionCheckRun";
import { viewDriver } from "./viewsCheckRun";
import type { FrameSubmission } from "../../../view/engine/types";
import { type SubmissionWatcher, viewOf, type ViewsProbe } from "./viewsProbe";

/** How long each part of the planet viewer run may take. */
export interface PlanetViewerCheckTiming {
  /** The longest wait for the link, and for a control to take effect, ms. */
  readonly waitMs: number;
  /** The longest wait for the planet's first photorealistic frames, lit, its air not pending, ms. */
  readonly drawWaitMs: number;
  /** How often a wait looks again, ms. */
  readonly pollMs: number;
  /** How far apart the `EXPOSURE` line's EV is read while `AUTO` settles, ms. */
  readonly settleStepMs: number;
  /** How near two readings in a row must be for `AUTO` to count as settled, EV. */
  readonly settleToleranceEv: number;
  /** The longest wait for `AUTO` to settle before a capture, ms: past it the run fails. */
  readonly settleMaxMs: number;
  /** The primary's draws waited for before a capture, and after each zoom. */
  readonly framesBeforeCapture: number;
  /** The longest wait for the ground to read surveyed, and for the terrain to stream, ms. */
  readonly groundWaitMs: number;
  /** The most zooms the camera is brought to a height by before the run fails. */
  readonly zoomTries: number;
  /**
   * The primary's draws `R` is held for, at least, to tilt the look to the horizon: each draw
   * turns it by at most `MAX_FREE_STEP_S` of its π/4 rad/s, so 25 turn it past π/2.
   */
  readonly tiltFrames: number;
  /** The least time `R` is held for, ms: the camera's two seconds to the horizon, with room. */
  readonly tiltMs: number;
}

/** The run's timing: a minute for a control, three for the first photorealistic frames. */
export const PLANET_VIEWER_CHECK_TIMING: PlanetViewerCheckTiming = {
  waitMs: 60_000,
  drawWaitMs: 180_000,
  pollMs: 100,
  settleStepMs: 1_000,
  settleToleranceEv: 0.1,
  settleMaxMs: 60_000,
  framesBeforeCapture: 5,
  groundWaitMs: 300_000,
  zoomTries: 6,
  tiltFrames: 25,
  tiltMs: 3_000,
};

/** The low orbit's height above the ground at the line of sight, m: 400 km, a station's. */
export const LOW_ORBIT_HEIGHT_M = 400_000;

/** The surface look's height above the ground at the line of sight, m: 300 m, a low flight's. */
export const SURFACE_HEIGHT_M = 300;

/** How near its goal a zoom must bring the camera, as a share of the goal's height: 10%. */
export const HEIGHT_TOLERANCE = 0.1;

/** The run's captures, in the order it takes them, each of the whole tab and `-maximized`. */
export const PLANET_VIEWER_CAPTURES = [
  "planet-viewer-orbit",
  "planet-viewer-low-orbit",
  "planet-viewer-surface",
  "planet-viewer-floor",
  "planet-viewer-moon",
] as const;

/** The most real planets the run draws seeking a system that holds a moon. */
export const MOON_SEARCH_DRAWS = 32;

/**
 * The first moon present at the epoch in `answer`'s system, by its ID, or `null` where it holds
 * none.
 */
export function firstMoon(answer: ResponseFor<"system_bodies">): BodyIdHex | null {
  const moon = answer.bodies.find(
    (body) => body.kind.type === "moon" && body.state.type === "present",
  );
  return moon?.id ?? null;
}

/**
 * The `INPUTS` column's captures (R15.T6's fold), of the whole tab, in the order the run takes
 * them: after the orbit's, `SHOWN` open, `ORBIT` and `PLANET` open where the compact layout folds
 * them (none in the full one), the volatile ranks open and the column's tallest state; after the
 * surface's, a server's refusal of `SMA`.
 */
export const PLANET_VIEWER_INPUTS_CAPTURES = [
  "planet-viewer-inputs-shown",
  "planet-viewer-inputs-orbit",
  "planet-viewer-inputs-planet",
  "planet-viewer-inputs-volatiles",
  "planet-viewer-inputs-tallest",
  "planet-viewer-inputs-refused",
] as const;

/** One of the run's captures. */
export type PlanetViewerCaptureLabel =
  (typeof PLANET_VIEWER_CAPTURES)[number] | (typeof PLANET_VIEWER_INPUTS_CAPTURES)[number];

/** The ground's height above the datum the floor's look is sought over, m: a high plain's. */
const HIGH_GROUND_M = 500;

/** The most moves over the ground the run makes seeking it, some 2,000 km at low orbit. */
const HIGH_GROUND_TRIES = 12;

/** The wheel's steps in that take the camera from 300 m above the ground down to its floor. */
const FLOOR_ZOOM_STEPS = 40;

/** A capture of a `RANDOMIZE` run's draw of the whole tab ({@link randomizeCaptureLabel}). */
export type RandomizeCaptureLabel = `planet-viewer-draw-${number}-${WhiteBalance}`;

/** One of the run's captures of the whole tab: a range's, or a `RANDOMIZE` draw's. */
export type PlanetViewerRunCapture = PlanetViewerCaptureLabel | RandomizeCaptureLabel;

/** The white balances each `RANDOMIZE` draw is captured under, in order. */
export const RANDOMIZE_BALANCES: ReadonlyArray<WhiteBalance> = ["host", "d65"];

/**
 * The capture of a `RANDOMIZE` run's draw `index` (from 1) under `balance`, of the whole tab:
 * `planet-viewer-draw-1-host`; its `-maximized` twin follows it.
 */
export function randomizeCaptureLabel(index: number, balance: WhiteBalance): RandomizeCaptureLabel {
  return `planet-viewer-draw-${index}-${balance}`;
}

/** A wheel's pixels in a zoom step (`ViewCanvas`'s `wheelSteps`, in pixels). */
const WHEEL_PX_PER_STEP = 100;

/** What the planet viewer run drives and reads. */
export interface PlanetViewerCheckDeps {
  readonly api: Pick<ViewsCheckApi, "capture">;
  /** The views probe: the primary's draws, and its submissions, by which a body is seen drawn. */
  readonly views: Pick<ViewsProbe, "drawTimes" | "watch">;
  /** The link's request client, through which the run lists the server's universes. */
  readonly requests: Pick<RequestClient, "request">;
  readonly document: Document;
  readonly nowMs: () => number;
  readonly sleep: (ms: number) => Promise<void>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly timing: PlanetViewerCheckTiming;
  /**
   * The `RANDOMIZE` draws to capture in place of the run's three ranges (`--randomize`), or none.
   */
  readonly randomize?: number | undefined;
}

/**
 * The ground's elevation above the datum at the line of sight as the lower plate's `ELEVATION` line
 * reads it (`ELEVATION ~2140 m ± 180 m`, `ELEVATION -1.25 km ± 0.02 km`), m, or `null` where it reads
 * none.
 */
export function elevationM(lines: ReadonlyArray<string>): number | null {
  for (const line of lines) {
    const match = /^ELEVATION ~?([+-]?[\d,]+(?:\.\d+)?) (m|km)\b/u.exec(line);
    const digits = match?.[1];
    if (digits !== undefined) {
      return Number(digits.replaceAll(",", "")) * (match?.[2] === "km" ? 1000 : 1);
    }
  }
  return null;
}

/**
 * The altitude above the datum a camera is brought to for `goalM` above ground standing `groundM`
 * above the datum, m: `goalM` above the ground, or above the datum where the ground lies below it,
 * a sea's floor perhaps, since `ORBIT` holds the camera above the datum.
 */
export function goalAltitudeM(groundM: number, goalM: number): number {
  return goalM + Math.max(groundM, 0);
}

/**
 * The wheel's zoom steps, nearer for a positive one, that bring a camera at `altitudeM` above the
 * datum to `goalAltitude` above it: `ORBIT` scales the altitude above the datum by
 * {@link ORBIT_ZOOM_FACTOR} a step, so the steps are the log of the altitudes' ratio.
 */
export function zoomStepsTo(altitudeM: number, goalAltitudeMetres: number): number {
  return Math.log(altitudeM / goalAltitudeMetres) / Math.log(ORBIT_ZOOM_FACTOR);
}

/**
 * Whether the primary's `STARS` reading says its sky has answered: anything but `PENDING`, which
 * stands from the sky's request until its first reply (`view/sky/label.ts`), a view that asks no
 * sky included.
 */
export function skyAnswered(starsLine: string | null): boolean {
  return starsLine !== SKY_PENDING;
}

/**
 * How far the console's frame overflows the window, CSS px, 0 where it fits or is not shown: the
 * guide's layout holds every display within the window, with no page scroll.
 */
export function consoleOverflowPx(
  frame: Pick<Element, "scrollHeight" | "clientHeight"> | null,
): number {
  return frame === null ? 0 : Math.max(frame.scrollHeight - frame.clientHeight, 0);
}

/**
 * How far the `INPUTS` panel's foot stands below its column's, CSS px, 0 where it fits or either is
 * not shown: the work area holds the column to the window and clips what passes it, so that a
 * panel too tall is cut there (the guide's Accessibility: nothing cut at the floor).
 */
export function inputsCutPx(
  panel: Pick<Element, "getBoundingClientRect"> | null,
  column: Pick<Element, "getBoundingClientRect"> | null,
): number {
  return panel === null || column === null
    ? 0
    : Math.max(panel.getBoundingClientRect().bottom - column.getBoundingClientRect().bottom, 0);
}

/** An entry out of every rank's range, which the panel refuses under its field. */
const REFUSED_RANK_ENTRY = "5";

/**
 * An `SMA` in range that the server refuses for the Sun at the panel's mount, AU: an orbit of
 * 0.001 AU, whose periapsis lies inside the star (the viewer's `PeriapsisInsideStar`).
 */
const REFUSED_SMA_ENTRY = "0.001";

/** A latitude out of `SUN`'s range, which the panel refuses under its field, °. */
const REFUSED_LATITUDE_ENTRY = "95";

/** The most of the primary's passes and materials a failure names. */
const NAMED_DRAWS = 24;

/**
 * The passes and materials the primary drew in its last frame, as the views probe tells its
 * submissions, which a capture's failure names: `bodies late: terrain-lit-cluster-…`.
 */
export class PrimaryDraws implements SubmissionWatcher {
  #frameMs: number | null = null;
  #draws = new Set<string>();

  submitted(
    owner: string,
    frame: Pick<FrameSubmission, "label" | "draws">,
    frameMs: number | null,
  ): void {
    if (frameMs === null || viewOf(owner) !== PRIMARY_VIEW_NAME) {
      return;
    }
    if (frameMs !== this.#frameMs) {
      this.#frameMs = frameMs;
      this.#draws = new Set();
    }
    for (const draw of frame.draws) {
      this.#draws.add(`${frame.label}: ${draw.material.name}`);
    }
  }

  madeTarget(): void {
    // Its targets say nothing of what it draws.
  }

  /** What the primary's last frame drew, at most {@link NAMED_DRAWS} of it, or `none`. */
  named(): string {
    const draws = [...this.#draws].slice(0, NAMED_DRAWS);
    return draws.length === 0 ? "none" : draws.join(", ");
  }
}

/** A key's press or release, dispatched on `target` as a keyboard would. */
function keyOn(target: EventTarget, type: "keydown" | "keyup", key: string): void {
  target.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true }));
}

/** A key's press and release on `target`. */
function pressKey(target: EventTarget, key: string): void {
  keyOn(target, "keydown", key);
  keyOn(target, "keyup", key);
}

/**
 * Runs the planet viewer run in `deps.document`'s consoles and returns its captures' labels,
 * watching the views probe's submissions for the lit bodies the primary draws.
 *
 * @throws Error naming the step where it cannot go on (the module's remarks).
 */
export async function runPlanetViewerCheck(
  deps: PlanetViewerCheckDeps,
): Promise<ReadonlyArray<PlanetViewerRunCapture>> {
  const bodies = new LitBodyDraws();
  const draws = new PrimaryDraws();
  const unwatch = deps.views.watch(bodies);
  const unwatchDraws = deps.views.watch(draws);
  try {
    return await viewPlanet(deps, bodies, draws);
  } finally {
    unwatchDraws();
    unwatch();
  }
}

/**
 * {@link runPlanetViewerCheck}'s steps, `bodies` the lit bodies the primary draws and `draws` what
 * its last frame drew.
 */
async function viewPlanet(
  deps: PlanetViewerCheckDeps,
  bodies: LitBodyDraws,
  draws: PrimaryDraws,
): Promise<ReadonlyArray<PlanetViewerRunCapture>> {
  const { document, timing } = deps;
  const { stopped, wait, until, press, controls } = viewDriver(deps);
  /** The captures taken so far, in order, each of the whole tab. */
  const taken: PlanetViewerRunCapture[] = [];
  const drawn = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.drawWaitMs } });
  const grounded = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.groundWaitMs } });
  const plate = (): PrimaryPlate => {
    const found = primaryPlate(document);
    if (found === null) {
      throw new Error("views check: the primary's label block is not shown");
    }
    return found;
  };
  const ground = (): ReadonlyArray<string> => lowerPlateLines(document);
  const maximized = (): boolean =>
    document.querySelector(".planet-viewer .view__side")?.hasAttribute("hidden") === true;
  /** Turns `MAXIMIZE VIEW` on or off by its key, as an operator does, and waits for it. */
  const setMaximized = async (on: boolean): Promise<void> => {
    if (maximized() === on) {
      return;
    }
    stopped();
    pressKey(document.body, MAXIMIZE_KEY);
    await until(
      `MAXIMIZE VIEW did not ${on ? "hide" : "show"} the panels`,
      () => maximized() === on,
    );
  };
  /** Waits for `count` of the primary's draws from now; returns when it began, ms. */
  const framesDrawn = async (count = timing.framesBeforeCapture, leastMs = 0): Promise<number> => {
    const fromMs = deps.nowMs();
    await drawn.until(
      `the primary did not draw ${String(count)} frames`,
      () =>
        deps.views.drawTimes(PRIMARY_VIEW_NAME, fromMs, Number.POSITIVE_INFINITY).length >= count &&
        deps.nowMs() - fromMs >= leastMs,
    );
    return fromMs;
  };
  /** Waits for `AUTO` to settle, from now, within the timing's limit. */
  const settle = async (): Promise<void> => {
    const fromMs = deps.nowMs();
    const read = (): number => {
      const line = plate().line("EXPOSURE");
      const ev = exposureEv100(line);
      if (ev === null) {
        throw new Error(`views check: the EXPOSURE line read no EV (${line ?? "none"})`);
      }
      return ev;
    };
    const settled = async (beforeEv: number): Promise<void> => {
      await wait(timing.settleStepMs);
      const ev = read();
      if (Math.abs(ev - beforeEv) <= timing.settleToleranceEv + 1e-9) {
        return;
      }
      if (deps.nowMs() - fromMs >= timing.settleMaxMs) {
        throw new Error(
          `views check: AUTO did not settle within ${String(timing.settleMaxMs / 1000)} s (EV100 ${beforeEv.toFixed(1)}, then ${ev.toFixed(1)})`,
        );
      }
      await settled(ev);
    };
    await settled(read());
  };
  /**
   * Waits for the server's warm-up, its sky's first reply, until `deadlineMs`; past it the run goes
   * on, saying so, for the ground's own wait to judge.
   */
  const warmedUp = async (deadlineMs: number): Promise<void> => {
    if (skyAnswered(plate().line("STARS"))) {
      return;
    }
    if (deps.nowMs() >= deadlineMs) {
      console.warn(
        `views check: the sky did not answer within ${String(timing.drawWaitMs / 1000)} s; the run goes on`,
      );
      return;
    }
    await wait(timing.settleStepMs);
    await warmedUp(deadlineMs);
  };
  /** The ground at the line of sight once it reads, under `MAXIMIZE VIEW` where it is held back. */
  const groundRead = async (what: string): Promise<ReadonlyArray<string>> => {
    if (readoutHeldBack(ground())) {
      await setMaximized(true);
    }
    await grounded.until(
      `the ground at the line of sight did not read ${what}`,
      () => inspectionGround(ground()) !== "waiting",
      () => ground().join("; "),
    );
    const lines = ground();
    if (inspectionGround(lines) === "not modelled") {
      throw new Error("views check: the planet's ground is not modelled, so it has no surface");
    }
    return lines;
  };
  /** Zooms the camera to `goalM` above the ground at its line of sight, by the wheel. */
  const zoomTo = async (goalM: number, what: string): Promise<void> => {
    const tryZoom = async (tries: number): Promise<void> => {
      const lines = await groundRead(what);
      const heightM = slantRangeM(lines);
      const groundM = elevationM(lines);
      if (heightM === null || groundM === null) {
        throw new Error(`views check: the lower plate reads no height (${lines.join("; ")})`);
      }
      const altitudeM = heightM + groundM;
      const goalAltitude = goalAltitudeM(groundM, goalM);
      if (!(altitudeM > 0)) {
        throw new Error(`views check: the camera stands at no altitude (${lines.join("; ")})`);
      }
      if (Math.abs(altitudeM - goalAltitude) <= HEIGHT_TOLERANCE * goalM) {
        return;
      }
      if (tries >= timing.zoomTries) {
        throw new Error(
          `views check: the camera did not come to ${String(goalM)} m above the ground ${what} (at ${String(Math.round(heightM))} m)`,
        );
      }
      const canvas = primaryCanvas(document);
      if (canvas === null) {
        throw new Error("views check: the primary's canvas is not shown");
      }
      stopped();
      const steps = zoomStepsTo(altitudeM, goalAltitude);
      canvas.dispatchEvent(
        new WheelEvent("wheel", {
          deltaY: -steps * WHEEL_PX_PER_STEP,
          deltaMode: 0,
          bubbles: true,
          cancelable: true,
        }),
      );
      await framesDrawn();
      await tryZoom(tries + 1);
    };
    await tryZoom(0);
  };
  /**
   * Fails unless the label block holds no fault and the lighting held, and, where `litBody` asks,
   * a lit body was drawn; else the lack of one is logged with what was drawn.
   */
  const shown = (what: string, fromMs: number, litBody: "required" | "logged"): void => {
    const said = plate();
    if (said.faults.length > 0) {
      throw new Error(`views check: ${what} shows faults: ${said.faults.join("; ")}`);
    }
    if (inspectionLighting(said.statements) !== "lit") {
      throw new Error(`views check: ${what} is not lit (${said.statements.join("; ")})`);
    }
    if (!bodies.drewSince(fromMs)) {
      const why = `${what} drew no lit body larger than a point (it drew ${draws.named()})`;
      if (litBody === "required") {
        throw new Error(`views check: ${why}`);
      }
      console.warn(`views check: ${why}`);
    }
  };
  /** Fails where the tab overflows the window, or the `INPUTS` column's foot is cut, `what`. */
  const fits = (what: string): void => {
    const overflowPx = consoleOverflowPx(document.querySelector(".console"));
    if (overflowPx > 0) {
      throw new Error(
        `views check: the PLANET VIEWER does not fit the window: it overflows by ${String(overflowPx)} px`,
      );
    }
    const cutPx = inputsCutPx(
      document.querySelector(".planet-inputs"),
      document.querySelector(".planet-viewer__inputs"),
    );
    if (cutPx > 0) {
      throw new Error(
        `views check: the INPUTS column is cut ${what}: its foot stands ${String(Math.ceil(cutPx))} px below the column's`,
      );
    }
  };
  /**
   * Shows the whole tab again, lets the terrain stream, `AUTO` settle and the frames come, checks
   * them and captures; then the same with `MAXIMIZE VIEW` on, and off again.
   */
  const capture = async (
    label: PlanetViewerRunCapture,
    what: string,
    litBody: "required" | "logged",
  ): Promise<void> => {
    await setMaximized(false);
    await grounded.until(
      `the terrain did not stream ${what}`,
      () => !plate().statements.includes("TERRAIN: STREAMING"),
      () => plate().statements.join("; "),
    );
    await settle();
    const fromMs = await framesDrawn();
    // Captured before it is judged, so that a failed run leaves the picture of what failed.
    stopped();
    await deps.api.capture(label);
    taken.push(label);
    shown(what, fromMs, litBody);
    fits(what);
    await setMaximized(true);
    const maximizedFromMs = await framesDrawn();
    stopped();
    await deps.api.capture(`${label}-maximized`);
    shown(`${what} under MAXIMIZE VIEW`, maximizedFromMs, litBody);
    await setMaximized(false);
  };

  /** The `INPUTS` panel's button whose text ends with `name`, a disclosure's or `SUN`'s. */
  const inputsButton = (name: string): HTMLButtonElement | null =>
    [...document.querySelectorAll<HTMLButtonElement>(".planet-inputs button")].find((button) =>
      button.textContent.trim().endsWith(name),
    ) ?? null;
  /** Opens or folds the `INPUTS` part `name` by its disclosure, as an operator does. */
  const setPartOpen = async (name: string, open: boolean): Promise<void> => {
    const expanded = (): boolean => inputsButton(name)?.getAttribute("aria-expanded") === "true";
    if (expanded() === open) {
      return;
    }
    await press(() => inputsButton(name), `INPUTS's ${name}`);
    await until(`INPUTS's ${name} did not ${open ? "open" : "fold"}`, () => expanded() === open);
  };
  /** Holds `SUN`, or gives it back to its orbit, by its buttons. */
  const setSunHeld = async (held: boolean): Promise<void> => {
    const word = held ? "HELD" : "ORBITAL";
    await press(() => inputsButton(word), `SUN's ${word}`);
    await until(
      `SUN's ${word} was not pressed`,
      () => inputsButton(word)?.getAttribute("aria-pressed") === "true",
    );
  };
  /** The `INPUTS` panel's field labelled `label`. */
  const inputsField = (label: string): HTMLInputElement | null => {
    const found = [...document.querySelectorAll<HTMLLabelElement>(".planet-inputs label")].find(
      (each) => each.textContent === label,
    );
    const field = found === undefined ? null : document.getElementById(found.htmlFor);
    return field instanceof HTMLInputElement ? field : null;
  };
  /** Types `text` in the field labelled `label` and enters it, or drops it, by its keys. */
  const entered = (label: string, text: string | null): void => {
    const field = inputsField(label);
    if (field === null) {
      throw new Error(`views check: INPUTS's ${label} field is not shown`);
    }
    stopped();
    field.focus();
    if (text === null) {
      pressKey(field, "Escape");
      field.blur();
      return;
    }
    // As a keyboard types it: React reads the value through the input event.
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(field, text);
    field.dispatchEvent(new Event("input", { bubbles: true }));
    pressKey(field, "Enter");
  };
  /** Captures the whole tab as `label` once the primary has drawn, and fails where INPUTS is cut. */
  const captureInputs = async (label: PlanetViewerCaptureLabel, what: string): Promise<void> => {
    await framesDrawn();
    stopped();
    await deps.api.capture(label);
    taken.push(label);
    fits(what);
  };

  /**
   * Moves the camera on over the ground under the arrows, from `tries` moves made, until the ground
   * at its line of sight, under it, stands {@link HIGH_GROUND_M} above the datum, at most
   * {@link HIGH_GROUND_TRIES} moves; past them the run goes on where it is, saying so.
   */
  const seekHighGround = async (tries: number): Promise<void> => {
    const groundM = elevationM(await groundRead("while seeking high ground"));
    if (groundM !== null && groundM >= HIGH_GROUND_M) {
      return;
    }
    if (tries >= HIGH_GROUND_TRIES) {
      console.warn(
        `views check: no ground ${String(HIGH_GROUND_M)} m above the datum found in ${String(tries)} moves`,
      );
      return;
    }
    await setMaximized(false);
    const canvas = primaryCanvas(document);
    if (canvas === null) {
      throw new Error("views check: the primary's canvas is not shown");
    }
    canvas.focus();
    stopped();
    keyOn(canvas, "keydown", "ArrowRight");
    try {
      await framesDrawn(timing.tiltFrames, timing.tiltMs);
    } finally {
      keyOn(canvas, "keyup", "ArrowRight");
    }
    await seekHighGround(tries + 1);
  };
  /** Opens the group `part` where it has a disclosure, and captures the tab as `label`. */
  const capturePart = async (part: string, label: PlanetViewerCaptureLabel): Promise<void> => {
    if (inputsButton(part) === null) {
      return;
    }
    await setPartOpen(part, true);
    await captureInputs(label, `with ${part} open`);
  };

  const randomizeButton = (): HTMLButtonElement | null =>
    document.querySelector<HTMLButtonElement>(
      `.planet-viewer__draw button[aria-keyshortcuts="${RANDOMIZE_KEY.toUpperCase()}"]`,
    );
  const lastDraw = (): string =>
    document.querySelector(".planet-viewer__last-draw .field__value")?.textContent ?? "";
  const shareString = (): string =>
    document.querySelector<HTMLInputElement>(".planet-viewer__string")?.value ?? "";
  const balanceButton = (balance: WhiteBalance): HTMLButtonElement | null =>
    [...document.querySelectorAll<HTMLButtonElement>(".view-exposure__balance button")].find(
      (button) => button.textContent === whiteBalanceName(balance),
    ) ?? null;
  const balanceRead = (balance: WhiteBalance): boolean =>
    plate().line(WHITE_BALANCE_LABEL)?.startsWith(whiteBalanceName(balance)) === true;
  /** Draw `index`'s planet captured under each of `balances` in turn; returns the labels. */
  const captureUnder = async (
    index: number,
    balances: ReadonlyArray<WhiteBalance>,
  ): Promise<ReadonlyArray<RandomizeCaptureLabel>> => {
    const [balance, ...rest] = balances;
    if (balance === undefined) {
      return [];
    }
    const name = whiteBalanceName(balance);
    await setMaximized(false);
    if (!balanceRead(balance)) {
      await press(() => balanceButton(balance), `WHITE BALANCE ${name}`);
      await until(`WHITE BALANCE did not read ${name}`, () => balanceRead(balance));
    }
    const label = randomizeCaptureLabel(index, balance);
    await drawn.until(
      `draw ${String(index)}'s atmosphere stayed pending`,
      () => !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
    );
    await capture(label, `draw ${String(index)} under WHITE BALANCE ${name}`, "logged");
    console.warn(
      `views check: ${label}: LAST DRAW ${lastDraw()}, WHITE BALANCE ${plate().line(WHITE_BALANCE_LABEL) ?? MISSING}, EXPOSURE ${plate().line("EXPOSURE") ?? MISSING}, ${shareString()}`,
    );
    return [label, ...(await captureUnder(index, rest))];
  };
  /** `RANDOMIZE` pressed for draws `index` to `count`, each captured under each white balance. */
  const randomizeDraws = async (
    index: number,
    count: number,
  ): Promise<ReadonlyArray<RandomizeCaptureLabel>> => {
    if (index > count) {
      return [];
    }
    await setMaximized(false);
    const before = lastDraw();
    // No block stands before the first planet is shown.
    const beforeFrame = primaryPlate(document)?.line("FRAME") ?? null;
    await until(
      "RANDOMIZE was held back",
      () => randomizeButton()?.getAttribute("aria-disabled") !== "true",
    );
    await press(randomizeButton, "RANDOMIZE");
    await drawn.until(
      `draw ${String(index)}'s planet was not shown`,
      () => lastDraw() !== before && lastDraw() !== MISSING,
    );
    // The new planet's scene, not the last one's still drawn: its frame names its own body.
    await drawn.until(
      `draw ${String(index)}'s scene was not drawn`,
      () => (primaryPlate(document)?.line("FRAME") ?? null) !== beforeFrame,
    );
    await drawn.until(
      `draw ${String(index)} was not drawn photorealistic`,
      () => controls.styleOf(PRIMARY_NAME) === "photorealistic",
    );
    await drawn.until(
      `draw ${String(index)}'s lighting and atmosphere stayed pending`,
      () =>
        inspectionLighting(plate().statements) !== "pending" &&
        !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
    );
    const labels = await captureUnder(index, RANDOMIZE_BALANCES);
    // Each draw is opened under HOST, the default, as the operator meets it.
    await setMaximized(false);
    await press(() => balanceButton("host"), "WHITE BALANCE HOST");
    await until("WHITE BALANCE did not read HOST", () => balanceRead("host"));
    return [...labels, ...(await randomizeDraws(index + 1, count))];
  };

  // 1. The first universe opened on GALAXY, and the PLANET VIEWER shown.
  const deadlineMs = deps.nowMs() + timing.waitMs;
  /** The server's universes, asked until the link answers, within the wait. */
  const listUniverses = async (): Promise<ReadonlyArray<UniverseInfo>> => {
    stopped();
    const listed = await deps.requests.request({ kind: "list_universes" }).outcome;
    if (listed.ok) {
      return listed.response.universes;
    }
    if (listed.error.code !== "link_lost" || deps.nowMs() > deadlineMs) {
      throw new Error(`views check: list_universes was refused: ${listed.error.message}`);
    }
    await wait(timing.pollMs * 10);
    return listUniverses();
  };
  const [universe] = await listUniverses();
  if (universe === undefined) {
    throw new Error("views check: the server lists no universe");
  }
  pressKey(document.body, "F2");
  const openButton = (): HTMLButtonElement | null =>
    document.querySelector<HTMLButtonElement>(
      `button[aria-label="Open universe ${CSS.escape(universe.name)}"]`,
    );
  await press(openButton, `GALAXY's OPEN UNIVERSE for ${universe.name}`);
  await until(`${universe.name} was not opened`, () => openButton() === null);
  pressKey(document.body, "F5");
  const generate = (): HTMLButtonElement | null =>
    document.querySelector<HTMLButtonElement>(
      `.planet-inputs button[aria-keyshortcuts="${GENERATE_KEY.toUpperCase()}"]`,
    );
  await until("the PLANET VIEWER was not shown", () => generate() !== null);
  if (deps.randomize !== undefined) {
    return randomizeDraws(1, deps.randomize);
  }

  // 2. The panel's inputs generated, drawn photorealistic from orbit, and captured.
  await until("GENERATE was held back", () => generate()?.getAttribute("aria-disabled") !== "true");
  await press(generate, "GENERATE");
  await drawn.until(
    "the planet was not drawn photorealistic",
    () => controls.styleOf(PRIMARY_NAME) === "photorealistic",
  );
  await drawn.until(
    "the planet's lighting and atmosphere stayed pending",
    () =>
      inspectionLighting(plate().statements) !== "pending" &&
      !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
  );
  await warmedUp(deps.nowMs() + timing.drawWaitMs);
  await groundRead("from orbit");
  await capture("planet-viewer-orbit", "the planet from orbit", "required");

  // 2b. The INPUTS column with SHOWN open, with the volatile ranks open, and at its tallest: the
  // ranks open, SHOWN too where the layout lets both stand, SUN HELD and a rank's entry refused.
  await setPartOpen("SHOWN", true);
  await captureInputs("planet-viewer-inputs-shown", "with SHOWN open");
  await setPartOpen("SHOWN", false);
  // The full layout stands ORBIT and PLANET open, with no disclosure, in every capture.
  await capturePart("ORBIT", "planet-viewer-inputs-orbit");
  await capturePart("PLANET", "planet-viewer-inputs-planet");
  await setPartOpen("VOLATILES · RANK", true);
  await captureInputs("planet-viewer-inputs-volatiles", "with the volatile ranks open");
  await setPartOpen("SHOWN", true);
  if (inputsButton("VOLATILES · RANK")?.getAttribute("aria-expanded") !== "true") {
    await setPartOpen("VOLATILES · RANK", true);
  }
  await setSunHeld(true);
  entered("H2O", REFUSED_RANK_ENTRY);
  await until(
    "the H2O entry was not refused",
    () => inputsField("H2O")?.getAttribute("aria-invalid") === "true",
  );
  await captureInputs("planet-viewer-inputs-tallest", "at its tallest");
  entered("H2O", null);
  await setSunHeld(false);
  await setPartOpen("SHOWN", false);
  await setPartOpen("VOLATILES · RANK", false);

  // 3. Down to low orbit, and captured.
  await zoomTo(LOW_ORBIT_HEIGHT_M, "in low orbit");
  await capture("planet-viewer-low-orbit", "the planet from low orbit", "required");

  // 3b. Over high ground, for the floor's look: the arrows move the camera on over the ground
  // until the ground under it stands HIGH_GROUND_M above the datum, or the tries run out.
  await seekHighGround(0);

  // 4. Down to the surface, the look tilted to the horizon, and captured.
  await zoomTo(SURFACE_HEIGHT_M, "near the surface");
  await setMaximized(false);
  const canvas = primaryCanvas(document);
  if (canvas === null) {
    throw new Error("views check: the primary's canvas is not shown");
  }
  canvas.focus();
  stopped();
  keyOn(canvas, "keydown", "r");
  try {
    await framesDrawn(timing.tiltFrames, timing.tiltMs);
  } finally {
    keyOn(canvas, "keyup", "r");
  }
  // Close over the ground the guard's materials may not be drawn at all, over a sea above all:
  // there it is logged with what was drawn, and the capture judged by its faults and lighting.
  await capture("planet-viewer-surface", "the planet's surface", "logged");

  // 5. Down to the floor over the ground under the camera, looking down at it, and captured.
  await setMaximized(false);
  canvas.focus();
  stopped();
  keyOn(canvas, "keydown", "f");
  try {
    await framesDrawn(timing.tiltFrames, timing.tiltMs);
  } finally {
    keyOn(canvas, "keyup", "f");
  }
  stopped();
  canvas.dispatchEvent(
    new WheelEvent("wheel", {
      deltaY: -FLOOR_ZOOM_STEPS * WHEEL_PX_PER_STEP,
      deltaMode: 0,
      bubbles: true,
      cancelable: true,
    }),
  );
  await framesDrawn();
  const floorLines = await groundRead("at the floor");
  const belowM = slantRangeM(floorLines);
  console.warn(`views check: at the floor the lower plate reads ${floorLines.join("; ")}`);
  if (belowM === null || belowM < ORBIT_FLOOR_M / 2) {
    throw new Error(
      `views check: at the floor the camera stands inside the ground or over none (${floorLines.join("; ")})`,
    );
  }
  await capture("planet-viewer-floor", "the planet's ground from the floor", "logged");

  // 6. A server's refusal of SMA, its reason under its field, with SUN HELD and LATITUDE refused.
  if (inputsButton("ORBIT") !== null) {
    await setPartOpen("ORBIT", true);
  }
  entered("SMA", REFUSED_SMA_ENTRY);
  await press(generate, "GENERATE");
  await until(
    "the server did not refuse SMA",
    () => inputsField("SMA")?.getAttribute("aria-invalid") === "true",
  );
  await setSunHeld(true);
  entered("LATITUDE", REFUSED_LATITUDE_ENTRY);
  await until(
    "the LATITUDE entry was not refused",
    () => inputsField("LATITUDE")?.getAttribute("aria-invalid") === "true",
  );
  await captureInputs("planet-viewer-inputs-refused", "with a server's refusal under SMA");

  // 7. A moon of the galaxy, opened by its ID with none of the inputs, captured from orbit.
  const moon = await moonOf(deps.requests, universe.id, stopped);
  await setSunHeld(false);
  await setPartOpen("SHOWN", true);
  entered("BODY", moon);
  const generateHeld = (): boolean =>
    generate()?.getAttribute("aria-disabled") === "true" &&
    [...document.querySelectorAll(".planet-inputs__note")].some(
      (note) => note.textContent === NO_INPUTS_REASON,
    );
  await drawn.until(`the moon ${moon} was not shown with NO INPUTS`, generateHeld);
  await setPartOpen("SHOWN", false);
  await drawn.until(
    "the moon was not drawn photorealistic",
    () => controls.styleOf(PRIMARY_NAME) === "photorealistic",
  );
  // Its photorealistic pipelines are made afresh for a new opening, the wireframe drawn meanwhile.
  await drawn.until(
    "the moon's photorealistic picture, its lighting and its atmosphere stayed pending",
    () =>
      !plate().statements.includes(PHOTOREAL_PREPARING) &&
      inspectionLighting(plate().statements) !== "pending" &&
      !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
  );
  await capture("planet-viewer-moon", `the moon ${moon} from orbit`, "required");
  return taken;
}

/**
 * The first moon of the real planets' systems `viewer_randomize` draws in `universe` from draw seed
 * 1 (the module's remarks, step 7).
 *
 * @throws Error where a request is refused, or no moon is found in {@link MOON_SEARCH_DRAWS} draws.
 */
async function moonOf(
  requests: Pick<RequestClient, "request">,
  universe: UniverseIdHex,
  stopped: () => void,
): Promise<BodyIdHex> {
  const searched = new Set<string>();
  for (let draw = 1; draw <= MOON_SEARCH_DRAWS; draw += 1) {
    stopped();
    const drawSeed = draw.toString(16).padStart(16, "0");
    // One after another: each draw's system is read before the next is drawn.
    // oxlint-disable-next-line no-await-in-loop
    const drawn = await requests.request({
      kind: "viewer_randomize",
      universe,
      draw_seed: drawSeed,
    }).outcome;
    if (!drawn.ok) {
      throw new Error(`views check: viewer_randomize was refused: ${drawn.error.message}`);
    }
    const { system } = parseBodyId(drawn.response.source);
    if (searched.has(system)) {
      continue;
    }
    searched.add(system);
    // The draw's system read before the next draw, which a moon found makes needless.
    // oxlint-disable-next-line no-await-in-loop
    const bodies = await requests.request(
      toBodiesRequest(universe, system, { seconds: 0, nanos: 0 }),
    ).outcome;
    if (!bodies.ok) {
      throw new Error(
        `views check: system_bodies of ${system} was refused: ${bodies.error.message}`,
      );
    }
    const moon = firstMoon(bodies.response);
    if (moon !== null) {
      return moon;
    }
  }
  throw new Error(
    `views check: no moon in the systems of ${String(MOON_SEARCH_DRAWS)} real planets drawn`,
  );
}

/** What {@link recordPlanetViewerCheck} asks of the main process. */
export type PlanetViewerCheckEnding = Pick<ViewsCheckApi, "end">;

/**
 * Runs `run` and ends the check with its outcome: passed once its captures are taken, or failed
 * with the reason it threw; nothing at all once `signal` has stopped it.
 */
export async function recordPlanetViewerCheck(
  api: PlanetViewerCheckEnding,
  run: () => Promise<ReadonlyArray<PlanetViewerRunCapture>>,
  signal: AbortSignal,
): Promise<void> {
  let failure: string | null = null;
  try {
    await run();
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    console.error("views check: the planet viewer run failed:", error);
    failure = error instanceof Error ? error.message : String(error);
  }
  if (signal.aborted) {
    return;
  }
  await api.end(failure === null ? { status: "pass" } : { status: "fail", reason: failure });
}
